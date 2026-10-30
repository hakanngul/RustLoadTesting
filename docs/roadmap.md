# RustLoad yol haritası: masaüstü yük testi özellikleri

Durum: taslak plan, uygulanmadı. Kapsam headless/CLI içermez; RustLoad masaüstü
uygulaması olarak kalır. Her madde ayrı, küçük bir PR olarak ilerler.

## Ortak kurallar (her PR için)

- **Geriye uyumluluk:** Eski plan JSON'ları, eski SQLite geçmişi ve eski eframe
  tercihleri açılmaya devam eder. Yeni alanlar `#[serde(default)]` ile eklenir;
  `SCHEMA_VERSION` yalnızca uyumsuz bir değişiklikte artar.
- **Ölçüm sözleşmesi:** `CLAUDE.md` ve `docs/decisions.md` içindeki kurallar sessizce
  değişmez. Sözleşmeye dokunan maddeler (warmup, aşamalar, açık model) kod, test ve
  belgeyi aynı PR'da günceller ve önceden onay ister.
- **Tek mantık:** Studio ve Studio 2 aynı editör/yardımcı fonksiyonları paylaşır.
- **Güvenlik:** Secret değerler, response body'leri ve ham hata metinleri (URL ve
  query değerleri içerebilir) plana, geçmişe, loga veya rapora yazılmaz.
- **Doğrulama:** fmt, strict clippy, test, build; davranış değişikliğine regresyon
  testi; `docs/validation.md` kaydı; UI değişikliğinde Dark/Light ve dar pencere
  kontrolü (Xvfb ekran görüntüsü).

## PR sırası

| # | PR | Faz | Büyüklük | Bağımlılık |
|---|----|-----|----------|------------|
| 1 | Uyumlu serileştirme + Byte/s | 0 | S | — |
| 2 | SLO eşikleri | 1 | M | 1 |
| 3 | Koşu karşılaştırma | 1 | M | 1 (2 ile daha iyi) |
| 4 | curl / HAR içe aktarma | 1 | M | — |
| 5 | Warmup fazı | 2 | S–M | 1 |
| 6 | Çok aşamalı yük profili (kapalı model) | 2 | M–L | — |
| 7 | Açık model (arrival rate) | 2 | L | 6 |
| 8 | Gecikme histogramı widget'ı | 3 | S–M | 1 |
| 9 | Hata detayı | 3 | M | 1 |
| — | OpenAPI içe aktarma | 1 (sonra) | M | 4 |

---

## Faz 0

### PR 1 — Uyumlu serileştirme ve Byte/s throughput

**Neden önce:** `Snapshot`, `Point`, `Counts`, `Latency` ve `Endpoint`
(`src/metrics.rs`) struct düzeyinde `#[serde(default)]` taşımıyor. Bunlara alan
eklemek, SQLite'taki eski özetlerin (`History::read`) ve noktaların
(`History::points`) okunmasını bozar. Sonraki her PR buna dayanır.

- `src/metrics.rs`: bu struct'lara `#[serde(default)]`.
- Byte sayacı: `Counts.bytes` (tam okunan response body byte'ları), `Point.bytes`,
  `Snapshot.bytes_rate` (RPS ile aynı son-5-saniye penceresi). reqwest sıkıştırma
  özelliği kapalı olduğu için body byte'ı kablodaki body byte'ına eşittir; header
  byte'ları dahil değildir. Bu tanım belgeye yazılır.
- UI: Live metrics kartı ve endpoint tablosunda kolon. Raporlar: JSON/HTML'de alan,
  CSV'de en sona yeni kolon (mevcut kolon sırası korunur).
- **Testler:** v0.1 biçiminde kaydedilmiş bir özet/nokta JSON'u yeni kodla okunur;
  fixture'ın bilinen body boyutlarıyla `bytes` toplamı birebir tutar.

## Faz 1 — Yüksek değer, düşük maliyet

### PR 2 — SLO eşikleri

- **Model (`src/core.rs`):** `TestPlan.thresholds: Vec<Threshold>`.
  `Threshold { metric, value, request_id: Option<u64> }`; `metric`: P50, P90, P95,
  P99, Mean (ms, "≤"), ErrorRate (%, "≤"), Throughput (istek/s, "≥"). Yön metrikten
  gelir, kullanıcı ayrıca operatör seçmez. `request_id` boşsa tüm koşu, doluysa o
  endpoint. Doğrulama: pozitif değer, var olan istek ID'si, en fazla 50 eşik.
- **Değerlendirme (`src/metrics.rs`):** saf fonksiyon
  `evaluate_thresholds(&Snapshot, &[Threshold]) -> Vec<ThresholdResult>`; her biri
  Pass / Fail / NoData. Yüzdelikler tüm-koşu histogramından; Throughput
  `completed / elapsed` (son-5-saniye RPS değil). Örnek yoksa NoData.
- **Karar:** `Snapshot.verdict: Option<Verdict>` (Passed / Failed / Inconclusive).
  Koşu bitince motor hesaplar ve özetle SQLite'a yazılır. Cancelled veya Failed
  koşularda karar "kısmi" olarak işaretlenir.
- **UI:** Plan ayarlarında eşik editörü (Studio ve Studio 2 ortak). Monitor ve Live
  metrics widget'ında canlı yeşil/kırmızı/gri rozetler, koşu sonunda büyük
  Passed/Failed başlığı. Canlı rozet "şu an" durumudur, nihai karar yalnızca sonda.
- **Raporlar:** HTML başlığında karar ve eşik tablosu; JSON'da eşikler + sonuçlar;
  CSV'de `slo_verdict` kolonu.
- **Testler:** değerlendirme tablosu (sınır değerler, NoData, endpoint kapsamı);
  fixture'ın 500 dönen yolu ile gerçek koşuda Failed kararı; eşiksiz eski plan açılır.

### PR 3 — Koşu karşılaştırma (baseline)

- **Veri:** SQLite şeması değişmez; iki koşu için `History::read` + `History::points`.
- **Saf modül `src/compare.rs`:**
  `compare(base, candidate, noise_pct) -> Comparison`. Satırlar: ortalama
  throughput (`completed / elapsed`), P50/P90/P95/P99, ortalama gecikme, hata oranı,
  Byte/s; her biri temel, aday, yüzde fark ve Improved / Regressed / Unchanged /
  NoData. Yön tablosu: gecikme ve hata oranında düşüş iyi, throughput'ta artış iyi.
  Endpoint satırları istek ID'siyle eşleşir; yalnızca bir tarafta olanlar ayrıca
  listelenir. Genel karar rlt'deki gibi: Improved / Regressed / Mixed / Unchanged.
- **Benzerlik uyarısı:** kullanıcı sayısı, süre, RPS limiti, profil, think time veya
  istek kümesi farklıysa ya da koşulardan biri Completed değilse "birebir
  karşılaştırma değil" uyarısı gösterilir; karşılaştırma engellenmez.
- **UI:** Monitor geçmiş listesinde "Karşılaştır" modu: bir koşu temel olarak
  sabitlenir, diğeri aday seçilir. Görünüm: zamanı 0'dan hizalanmış üst üste
  throughput ve P95 grafikleri, fark tablosu, karar. Gürültü eşiği eframe
  tercihlerinde tutulur (plana girmez).
- **Not:** Gürültü eşiği istatistiksel anlamlılık testi değildir; belgede açıkça
  yazılır.
- **Testler:** yön, gürültü sınırı, sıfır temel değer, eksik endpoint, farklı config
  uyarısı.

### PR 4 — curl ve HAR içe aktarma

- **Saf modül `src/import.rs`:** `from_curl(&str)`, `from_har(&[u8])` →
  `Vec<Request>` + uyarı listesi.
- **curl:** Yeni bağımlılık olmadan kendi shell tokenizer'ı (tek/çift tırnak, `\`,
  satır devamı; Chrome/Firefox "Copy as cURL" biçimleri). Desteklenen bayraklar:
  `-X`, `-H`, `-d/--data/--data-raw/--data-binary`, `--json`, `-u`, `-G`, `--url`,
  `-b` (Cookie header'ı olarak). `-k/--insecure` desteklenmez (TLS doğrulaması hep
  açık), uyarı verilir. Bilinmeyen bayraklar uyarı listesine düşer.
- **HAR:** serde ile. Yalnızca desteklenen metotlar ve http(s). "Yalnızca XHR/fetch"
  seçeneği (`_resourceType` varsa), statik dosyaları ayıklar. Sıra korunur.
- **Header temizliği:** HTTP/2 pseudo-header'ları (`:authority`), hop-by-hop
  header'lar, `content-length` ve `accept-encoding` atılır. Sıkıştırma kapalı
  olduğundan `accept-encoding` kalırsa body ölçümleri ve extraction bozulur.
- **Güvenlik:** `Authorization`, `Cookie` ve hassas query parametreleri secret alan
  olur (değer yalnızca oturumda kalır). Host literal kalır. İçe aktarmadan sonra
  plan dirty olur ve hedef yetkilendirme onayı sıfırlanır. En fazla 500 istek;
  dosya boyutu sınırı.
- **UI:** İçe aktarma diyaloğu (curl için yapıştırma alanı, HAR için dosya yolu),
  "mevcut plana ekle / planı değiştir" seçimi, uyarı önizlemesi.
- **Testler:** gerçek tarayıcı çıktılarından curl örnekleri, küçük bir HAR fixture'ı,
  içe aktarılan secret'ın kaydedilen plana yazılmadığı test.
- **OpenAPI ayrı PR:** JSON biçimi yeni bağımlılık gerektirmez; YAML için
  `serde_yaml` gerekir (karar bekliyor).

## Faz 2 — Ölçüm doğruluğu

### PR 5 — Warmup fazı

- `LoadConfig.warmup_secs` (varsayılan 0). Süre warmup'ı içerir (ramp ile tutarlı).
- **Kural:** warmup bitmeden *başlayan* istekler sonuçlara hiç girmez (sayaç,
  histogram, status, endpoint). Ayrı `warmup` sayaçları tutulur. Böylece
  `started = completed + cancelled + in_flight` her faz için ayrı ayrı korunur.
- Seri noktalarına faz bilgisi; grafiklerde warmup bölgesi gölgeli; RPS penceresi
  warmup'ı saymaz. Loglar warmup etiketiyle kalır.
- Ölçüm sözleşmesi değişikliği: `docs/decisions.md` ve `CLAUDE.md` güncellenir.
- **Testler:** sabit RPS ile `started ≈ rps × (süre − warmup)`; warmup sayaçları
  ayrı; warmup = 0 iken davranış birebir aynı.

### PR 6 — Çok aşamalı yük profili (kapalı model)

- `LoadConfig.stages: Vec<Stage { duration_secs, target_users }>`. Mevcut Constant
  ve RampUp profilleri tek bir dönüştürücüyle aşamalara çevrilir; motorda tek kod
  yolu kalır, eski planlar aynı davranışla açılır.
- **Motor:** kullanıcı sayısını her tikte doğrusal interpolasyonla hedefleyen bir
  denetleyici task. Artışta yeni kullanıcı başlatılır. Azalışta kullanıcıya ait
  child `CancellationToken` ile durması istenir; kullanıcı mevcut iterasyonu
  bitirince çıkar. CSV satırı kullanıcı indeksine bağlı kalır.
- Doğrulama: en fazla 20 aşama, toplam ≤ 86.400 s, ≤ 10.000 kullanıcı.
- **UI:** egui_plot'ta sürüklenebilir noktalı aşama grafiği ve sayısal tablo
  (`CLAUDE.md`'deki "sürüklemeye alternatif kontrol" kuralı). Ortak editör.
- **Testler:** kontrol noktalarında aktif kullanıcı sayısı hedefe ±1 uyar;
  azalışta kullanıcılar iterasyon bitince çıkar; eski RampUp planı aynı sonucu
  verir.

### PR 7 — Açık model (arrival rate)

- Yeni profil: hedef iterasyon/s aşamaları + `max_vus`. Zamanlayıcı iterasyon
  başlangıçlarını takvime göre üretir; her biri boştaki bir sanal kullanıcıya
  verilir; boş yoksa ve havuz `max_vus`'tan küçükse yeni kullanıcı açılır; değilse
  iterasyon **dropped** sayılır (k6 ile aynı anlam) ve arayüzde öne çıkarılır.
- Sunucu yavaşlasa da başlangıçlar takvime bağlı olduğu için gecikme olduğundan iyi
  görünmez (coordinated omission). Gecikme tanımı değişmez; dropped sayacı eklenir.
- Kullanıcının durumu (çıkarılmış token, CSV satırı) kendi iterasyonları arasında
  korunur.
- Bu modda global RPS limiti devre dışıdır (hız arrival rate ile belirlenir);
  doğrulama bunu açıkça belirtir.
- **Testler:** gecikmeli fixture endpoint'inde kapalı ve açık model farklı
  davranır; küçük `max_vus` ile dropped sayılır; takvim doğruluğu (PR 1'deki pacing
  testine benzer).

## Faz 3 — Hata ayıklama ve görünürlük

### PR 8 — Gecikme histogramı widget'ı

- `Snapshot.latency_buckets`: tüm-koşu HDR histogramından sabit, log ölçekli
  yaklaşık 40 kova (üst sınır ms, adet). Boyut sınırlı.
- `WidgetKind::LatencyHistogram`, `Unknown`'dan önce eklenir; katalog 8 olur. Eski
  tercihler etkilenmez; katalog sınırı testleri güncellenir.
- Bimodal dağılımı gösterir; P50/P95/P99 çizgileri üzerine işaretlenir.
- **v2 (opsiyonel):** heatmap için her saniyelik noktaya kova sayıları; bellekte
  900 × 40 ve SQLite'ta saniyede yaklaşık 200 byte ek yük. Ayrı karar.

### PR 9 — Hata detayı

- `Failure` sınıflandırılmış ve temizlenmiş nedenler taşır: Network (connect, DNS,
  TLS, reset, body), Timeout, HTTP status, Assertion (status, max_ms, JSON pointer,
  extractor değişken adı), Template (alan adı, değer değil).
- reqwest hata metni hiçbir yerde kullanılmaz (URL ve query değerlerini içerir).
- Accumulator'da `(request_id, neden)` anahtarlı gruplar: adet, ilk/son görülme,
  sınırlı saniyelik sparkline; en fazla 200 grup.
- **UI:** gruplu hata tablosu; tıklanınca ilgili istek seçilir ve zaman çizelgesi
  açılır. JSON/HTML raporlarına eklenir.
- **Testler:** her neden doğru grupta; secret işaretli bir query değerinin
  snapshot/rapor JSON'unda hiç görünmediği test.

---

## Karar bekleyen sorular

1. **SLO ihlali:** yalnızca karar mı üretsin, yoksa opsiyonel olarak koşuyu
   durdurabilsin mi? (Öneri: v1'de yalnızca karar.)
2. **Karşılaştırma:** farklı ayarlarla koşulmuş iki koşu uyarıyla karşılaştırılabilsin
   mi? (Öneri: evet.) Varsayılan gürültü eşiği? (Öneri: %2.)
3. **OpenAPI:** YAML desteği için `serde_yaml` bağımlılığı eklensin mi? (Öneri: önce
   curl + HAR, OpenAPI sonra.)
4. **Warmup:** süre bazlı mı, istek sayısı bazlı mı? (Öneri: süre.)
5. **Arrival rate:** bu modda RPS limiti devre dışı mı kalsın? (Öneri: evet.)
6. **Ölçüm sözleşmesi:** PR 5–7 için `CLAUDE.md` ve `docs/decisions.md`
   güncellemesine onay.
7. **Bu dosya:** repoya commit edilsin mi? Diğer belgeler gibi İngilizceye
   çevrilmeli mi?
