# RustLoad — MVP Product Requirements Document (PRD)

> **Hedef okuyucu:** Codex (uygulama geliştirici ajan)\
> **Ürün:** RustLoad — yerel masaüstü HTTP yük testi uygulaması\
> **Sürüm hedefi:** MVP v0.1.0\
> **Durum:** Ürün ve mimari gereksinimleri; kaynak kod değildir\
> **Dil:** Ürün arayüzünde İngilizce varsayılan, mimari ve uygulama kararlarında Türkçe açıklama kabul edilir.

---

## 0. Codex için çalışma talimatı

Sen deneyimli bir **Rust Software Architect, Performance Test Engineer ve Desktop UX Engineer** olarak çalışıyorsun. Bu dokümanı uygulamanın **tek kaynak ürün gereksinimi** olarak ele al.

1. Önce çalışma dizinini ve varsa mevcut repository'yi incele; mevcut işleri silme veya üzerine yazma. Mevcut bir proje varsa onun yapısına uyum sağla.
2. Kodlamadan önce kısa bir **mimari tasarım + dosya/modül planı + risk listesi** çıkar. Gereksiz onay beklemeden bu PRD'nin onaylanmış sınırları içinde MVP'yi uygulamaya geç.
3. **Tamamen Rust** kullan. UI için **egui + eframe** zorunlu. **React, TypeScript, Tauri, Electron, HTML/WebView** kullanma.
4. Ürünü iki ana çalışma alanında oluştur: **Studio** ve **Monitor**. **Flow** ekranını veya node builder'ı MVP'ye ekleme.
5. Gerçek HTTP yük testi gerçekleştirmeyen mock dashboard oluşturup "MVP tamamlandı" deme. Kullanıcının başlattığı test, Rust motorundan gerçek istekler göndermeli ve gerçek ölçümler üretmeli.
6. Her aşamada derleme, test ve lint kontrollerini çalıştır. Mümkün olmayan doğrulamaları açıkça belirt; hiçbir test sonucunu uydurma.
7. **Hedef yetkilendirmesi / güvenli çalışma** tasarımı önceliklidir. Kullanıcının kontrol etmediği sistemlere otomatik yüksek yük gönderme; açık hedef onayı, sınırlı varsayılanlar, durdurma ve kaynak sınırları uygula.
8. İlk hedef **macOS** üzerinde çalışır MVP; mimari Windows ve Linux'a taşınabilir olmalı. Platform desteğini ancak gerçekten doğrulandıysa “test edildi” diye belirt.
9. Geliştirme sırasında `docs/architecture.md`, `docs/decisions.md`, `docs/test-plan.md` ve `README.md` üret; değişikliklerin nedenini ve kabul kriterlerini takip et.
10. Her kilometre taşından sonra tamamlanan/kalan işleri, çalıştırılan komutları ve doğrulama sonuçlarını özetle.

## 1. Ürün vizyonu

RustLoad, JMeter benzeri bir **masaüstü HTTP yük testi uygulamasıdır**. Kullanıcı kendi bilgisayarında programı açar, hedef URL ve istekleri tanımlar, sanal kullanıcı/load profilini ayarlar, testi başlatır ve sonuçları canlı izler. Sunucu, Docker, hesap, bulut servisi veya harici backend zorunlu değildir.

**Ürün vaadi:** JMeter'ın temel HTTP test planlama yetenekleri + Locust benzeri kullanıcı/yük modeli + modern, hafif ve ölçülebilir bir Rust desktop uygulaması.

### 1.1 Hedef kullanıcılar

- QA / SDET / Performance Test Engineer
- Backend developer
- API servisinin yük altında davranışını yerel olarak test eden küçük ekipler

### 1.2 Temel ilkeler

- **Local-first:** test planları ve geçmişi cihazda saklanır.
- **Gerçek ölçüm:** dashboard değerleri yürütülen testlerden gelir; örnek veri yalnızca demo modunda açıkça işaretlenir.
- **UI/engine ayrımı:** desktop arayüzü load engine'i bloklamaz; motor ileride CLI tarafından da kullanılabilir.
- **Basit başlangıç, derin kontrol:** bir URL ile ilk testi başlatmak kolay; gelişmiş headers/auth/assertion alanları erişilebilir.
- **Kaynak farkındalığı:** yük üreten cihazın sınırlamaları görünür ve kontrol altındadır.

## 2. Kapsam

### 2.1 MVP kapsamına dahil

**Studio**
- Test planı oluştur, aç, güncelle, kaydet, farklı kaydet.
- Ağaç görünümü: Test Plan → User Group → HTTP Requests → Assertions / Data Sources.
- Bir planda birden çok HTTP request; sıralı senaryo akışı.
- HTTP metotları: GET, POST, PUT, PATCH, DELETE.
- URL, query params, headers, body (JSON / raw text), timeout.
- Basic ve Bearer Auth; token değeri saklama için gizlilik önlemleri.
- Değişkenler: plan/global ve sanal kullanıcı kapsamı; `{{variable}}` placeholder çözümlemesi.
- JSON response'dan alan çıkarma (JSONPath veya belgelenmiş eşdeğer) ve sonraki isteğe aktarma.
- CSV veri kaynağı (en az: başlık satırı, sanal kullanıcıya satır atama, dosya yolu doğrulama).
- Assertions: HTTP status, JSON alanı ve üst süre limiti.
- Test ayarları: sanal kullanıcı sayısı, ramp-up süresi, toplam test süresi, per-request timeout, `think time` (sabit), güvenli global RPS limiti.
- Yük profilleri: **Constant Users** ve **Ramp-up Users**.
- Start, Stop, Reset; çalışmadan önce plan doğrulaması.
- Test planı için yerel JSON serialize/deserialize.

**Monitor**
- Aktif/hedef sanal kullanıcı, tamamlanan istek sayısı, RPS, başarı/hata sayısı ve oranları.
- P50/P90/P95/P99 latency (global ve request/endpoint bazında).
- Canlı throughput, latency ve sanal kullanıcı grafikleri.
- HTTP status dağılımı ve en yavaş endpoint'ler.
- Son olaylar ve sınırlı, filtrelenebilir log akışı; hataların kök nedeni için örnek detay.
- Test süresi ve durumları: Idle, Validating, Running, Stopping, Completed, Failed, Cancelled.
- Önceki testlerin SQLite üzerinde yerel listelenmesi; seçilen koşunun özetini açma.
- HTML + JSON + CSV rapor dışa aktarma.

**Arayüz / platform**
- Dark + Light tema, manuel geçiş; tema tercihi kalıcı.
- Referans tasarım: sol test planı, orta editör, sağ metrik paneli, altta grafik/loglar; Monitor ekranında tam genişlik analitik düzen.
- Yeniden boyutlandırılabilir paneller, temel klavye kısayolları, açıklayıcı boş/hata durumları.
- Uygulama kapanırken planın kaydedilmemiş değişiklikleri için uyarı.

### 2.2 MVP dışında

- Flow / sürükle-bırak node editor
- Gerçek tarayıcı emülasyonu veya browser RUM ölçümü
- WebSocket, gRPC ve TCP/UDP özel protokol yük testi
- Distributed worker ve cloud orchestration
- AI senaryo üretimi veya AI analiz
- Kullanıcı hesabı, ekip paylaşımı, SaaS backend
- JMeter `.jmx` içe aktarma / tam uyumluluk
- Karmaşık scripting dili, plugin marketplace, proxy recorder
- PDF rapor, otomatik benchmark karşılaştırma ve regression gates

## 3. Kullanıcı hikâyeleri ve kabul kriterleri

| ID | Kullanıcı hikâyesi | Kabul kriteri |
|---|---|---|
| US-01 | Yeni HTTP test planı oluşturmak istiyorum | New Test → URL + method + user/duration → Run ile gerçek HTTP isteği gider |
| US-02 | Çok adımlı API senaryosu kurmak istiyorum | Login → token extraction → authenticated GET → assertion, aynı sanal kullanıcı bağlamında çalışır |
| US-03 | Sanal kullanıcı yükünü kontrol etmek istiyorum | Constant ve ramp-up kullanıcı hedefi izlenir; belirtilen RPS üst sınırı uygulanır |
| US-04 | Testi güvenle durdurmak istiyorum | Stop yeni iş başlatmayı keser, devam edenleri timeout/grace period ile sonlandırır, kısmi rapor üretir |
| US-05 | Hataları görmek istiyorum | HTTP hataları, network/timeout hataları ve assertion başarısızlıkları ayrı tanımlanır |
| US-06 | Sonuçları incelemek istiyorum | Monitor canlı sayılar/grafikler gösterir; test sonu özetini saklar |
| US-07 | Planı yeniden açmak istiyorum | JSON olarak kaydedilen plan geri yüklenince istek sırası ve ayarları korunur |
| US-08 | Test geçmişini raporlamak istiyorum | Geçmiş test açılır; HTML/JSON/CSV dışa aktarılır |
| US-09 | Temayı değiştirmek istiyorum | Light/Dark her iki ekranda tutarlı görünür, restart sonrası kalır |
| US-10 | Yük üreticimin sınırlarını bilmek istiyorum | Uygulama local CPU/RAM kullanımı ve limit uyarısını gösterir; metrikler ölçüm hedefiyle karıştırılmaz |

## 4. Ana kullanım akışı

1. Kullanıcı RustLoad'u masaüstünde açar.
2. **Studio** içinde yeni test planı oluşturur veya JSON planı açar.
3. URL/HTTP method, request detayları, assertion, değişken ve kullanıcı profili belirler.
4. Hedef sistemde test yapma yetkisi olduğunu açıkça onaylar; uygulama planı doğrular.
5. **Run Test** ile yerel Rust engine çalışır.
6. **Monitor** otomatik açılabilir; canlı metrikler örneğin 500 ms–1 s aralığında özetlenerek gösterilir.
7. Kullanıcı Stop/Pause gereksinimine göre **MVP'de yalnızca Stop** ile testi kesebilir; Pause sonraki sürüm olarak gizlenir.
8. Test bittikten sonra sonuç SQLite geçmişine yazılır, rapor dışa aktarılabilir.

## 5. Ekran gereksinimleri

### 5.1 Studio

**Üst bar:** RustLoad logo, Studio/Monitor geçişi, New, Open, Save, Run, Stop, Export, tema seçimi. Pasif butonlar uygun durumda devre dışı.

**Sol panel — Test Plan Tree:** Plan adı; User Group; sıralı HTTP request'ler; assertions ve CSV kaynakları. Ekle/sil/kopyala/yeniden adlandır ve sıralama aksiyonları. Seçilen node orta editörde açılır.

**Orta panel — Request Editor:** Name, HTTP method, URL; Params / Headers / Body / Auth / Assertions / Extractors / Advanced sekmeleri. Hata mesajları ilgili alanda gösterilir. Bearer token gibi hassas veriler ekranda gerektiğinde gizlenir.

**Sağ panel — Live Metrics/Configuration:** Test duruyorsa load configuration; test çalışıyorsa aktif kullanıcı, RPS, P95, error rate. Konfigürasyonun test süresince değiştirilmesi engellenir.

**Alt panel:** Throughput chart, latency chart, filtrelenebilir loglar. Paneller yeniden boyutlandırılabilir; küçük pencerede alanlar sekmeli gösterilebilir.

### 5.2 Monitor

**Üst:** Test adı, durum, geçen süre, Stop, Report Export.

**Metrik kartları:** Active/Target Users, RPS, P50, P95, P99, Error Rate, Request Count.

**Grafikler:** RPS zaman serisi; P50/P95/P99 latency; aktif kullanıcı; HTTP status dağılımı; latency histogram. MVP'de grafiklerde en az zaman serileri ile status dağılımı bulunmalı; histogram yapılabiliyorsa eklenir ancak doğruluğu garanti edilmeden gösterilmez.

**Alt tablolar:** Endpoint breakdown (method + path, count, errors, RPS, P95), olay/log tablosu, test geçmişi. Log saklama ve ekranda çizim için bounded ring buffer kullan.

**Not:** Dashboard hiçbir zaman sabit mock sayılar göstermesin. Örnek mod varsa çok belirgin "Demo" etiketi kullan.

## 6. Design System — Referans Studio/Monitor görselleri

Tasarım dili: **koyu lacivert/antrasit yüzeyler, belirgin turuncu birincil aksiyon, yumuşak sınırlar, küçük ve yoğun veri panelleri**. Doğrudan uygulama arayüzünde renklerin tamamı semantic token'lardan gelsin.

### 6.1 Dark theme

| Token | HEX |
|---|---|
| background | `#131A20` |
| surface | `#1B2229` |
| surface-elevated | `#232C35` |
| border | `#303B46` |
| text-primary | `#F3F6FA` |
| text-secondary | `#A5B1BD` |
| accent-primary | `#FF873E` |
| selection | `#3D2C23` |
| success | `#3DDC91` |
| error | `#FF5E68` |
| chart-p50 | `#4EA9FF` |
| chart-p95 | `#B085FF` |
| chart-p99 | `#FF4F97` |

### 6.2 Light theme

| Token | HEX |
|---|---|
| background | `#F5F7FA` |
| surface | `#FFFFFF` |
| surface-elevated | `#EAF0F4` |
| border | `#D7DFE7` |
| text-primary | `#18222C` |
| text-secondary | `#52616F` |
| accent-primary | `#BD5419` |
| selection | `#FCE8DC` |
| success | `#137E50` |
| error | `#BE3142` |
| chart-p50 | `#1668C1` |
| chart-p95 | `#7951B4` |
| chart-p99 | `#B92368` |

### 6.3 Ölçüler ve etkileşim

- Font: Inter veya okunaklı sistem fontu, lisans ve paketleme gereksinimini kontrol et.
- Normal metin: 13–14 px; ikincil açıklama: 11–12 px; panel başlığı: 16–20 px; metrik değeri: 24–30 px.
- Border: 1 px; kart radius: 10 px; panel radius: 14 px; panel padding: 12 px; gap: 8 px.
- Grafik ve durum anlamlarını yalnızca renk üzerinden iletme; ikon, etiket ve çizgi açıklaması kullan.
- Testi durdurma ve hedef izni gibi kritik kontroller net görünür olmalı.
- Eğer referans görseller repo içinde mevcutsa renk/yerleşim karşılaştırması yap. Görsellere erişilemiyorsa bu PRD'deki tasarım tanımları yeterlidir.

## 7. Teknik mimari

**Zorunlu teknoloji:** Rust stable, `egui`, `eframe`, `tokio`, `serde`, `serde_json`. Önerilen: `reqwest` (ilk HTTP motoru için), `hdrhistogram`, `egui_plot`, `rusqlite`, `tracing`, `thiserror`. Bu kütüphanelerin güncel uyumlu sürümlerini geliştirme sırasında doğrula; kararlı sürümleri sabitle, lockfile tut. İleride gerektiğinde HTTP katmanı `hyper` ile optimize edilebilir; ilk MVP'de gereksiz düşük seviye karmaşıklık ekleme.

### 7.1 Rust workspace düzeni (öneri)

```text
rustload/
├── Cargo.toml
├── Cargo.lock
├── crates/
│   ├── rustload-core/         # test plan modeli, validation, settings
│   ├── rustload-engine/       # async virtual users, HTTP, pacing, stop
│   ├── rustload-metrics/      # counters, histograms, rollups
│   ├── rustload-storage/      # local SQLite, plan/report persistence
│   ├── rustload-report/       # HTML/JSON/CSV export
│   └── rustload-desktop/      # egui/eframe Studio + Monitor
├── examples/                 # local-only example APIs and plans
├── tests/                    # cross-crate integration tests
├── docs/
│   ├── architecture.md
│   ├── decisions.md
│   └── test-plan.md
└── README.md
```

Modül isimleri örnektir; bağımlılık yönü önemlidir: **Desktop → Engine/Core/Metrics/Storage/Report**. Core ve Engine, UI'dan bağımsız olacak. Engine içinde `egui` bağımlılığı olmayacak.

### 7.2 Engine model ve ölçümleri

- **Virtual user = bağımsız async task / durum bağlamı.** Tek bir kullanıcı içindeki istekler belirlenen sırayla çalışır; değişken/token state kullanıcılar arasında yanlışlıkla paylaşılmaz.
- **Closed-loop concurrent-users modeli:** Her kullanıcı senaryosunu tamamlayınca duruş koşulları izin verirse yeniden yürütür. Ramp-up, aktif task/user sayısını yükseltir; global RPS üst sınırı ayrıca pacing uygular.
- **RPS limit**, sanal kullanıcı sayısına eşit değildir. User count ve arrival rate farklı ölçümlerdir; UI'da bu ayrımı açıkça anlat.
- Her istek için `request_id`, timestamp, duration, method, endpoint label, HTTP status veya network error tipi, assertion sonucu üretilir.
- Latency: request başlangıcından cevabın **tamamının okunduğu ana kadar** geçen wall-clock süre; bu tanımı metrik ve raporlarda açıkla. İlk MVP için bu yeterli, TTFB ayrı metrik değildir.
- Başarı ölçütü: Varsayılan olarak ağ seviyesinde başarılı ve assertion'ları geçen istek. HTTP 4xx/5xx ve assertion başarısızlıkları ayrı sayaçlarda izlenir; kullanıcı tarafından beklenen HTTP statüleri assertion ile tanımlanabilir.
- Histogram/percentile, **tamamlanmış istek örnekleminden** hesaplanır. P50/P90/P95/P99 ayrımı her zaman aynı ölçüm tanımına dayanmalı.
- 500 ms–1 sn aralıklarla aggregate snapshot üret; **her request'i UI'ye IPC/event olarak basma**. Ring buffer / bounded channel ile bellek kullanımını sınırla.
- Async runtime UI thread'den ayrı yönetilmeli; start/stop race condition, cancellation, cleanup ele alınmalı.
- Her test run için immutable config snapshot sakla; UI editörü çalışırken config'i değiştiremesin.
- Bağlantı yeniden kullanımı/pooling ve timeout ayarları açıkça belgelenmeli. Çalışma sırasında `429 Too Many Requests` ve diğer 4xx/5xx durumları raporlanmalı; agresif otomatik retry varsayılan olarak kapalı.
- Ölçüm doğruluğu için **coordinated omission**, client saturation ve load generator kapasite sınırlarını risk olarak belgele. Açık/arrival-rate modelini bu MVP'ye ekleme.

### 7.3 Saklama ve rapor

- Plan JSON formatına `schema_version` ekle; migration stratejisi belgele.
- SQLite: test_run metadata, zaman serisi aggregate snapshot'ları, endpoint summary, error counters, config snapshot.
- Her isteğin ham yanıt gövdesini kalıcı saklama (bellek, disk ve gizlilik riski); yalnızca gerektiğinde boyutu ve hassas verileri sınırlanmış örnek hata bilgisi sakla.
- HTML rapor offline açılabilsin; JSON ve CSV machine-readable olsun. Rapor, testin tamamlanmış mı iptal edilmiş mi olduğunu belirtmeli.

### 7.4 Güvenlik ve mahremiyet

- Yalnızca kullanıcının yetkili olduğu hedeflere test göndermesini sağlayan açık uyarı/onay akışı.
- Varsayılan düşük yük: ör. 10 user, 30 saniye, düşük global RPS üst sınırı; daha yüksek değerler için bilinçli ayar.
- URL validation: yalnız `http://` ve `https://`; timeout ve redirect sınırları. Lokal loopback kullanımını destekle.
- UI hata/log/raporlarda `Authorization` header'ı, token, password, cookie ve diğer sırlar redakte edilmeli.
- URL içinde parola/token içeren query parametreleri için uyarı/redaction.
- Durdurmanın güvenilir olması; test sonunda worker/task temizliği.
- Kullanıcının CPU/RAM ve bağlantı kapasitesini aşırı tüketen senaryolar için limit ve uyarı.
- Sadece test amacıyla yetkili olunan sistemleri hedefle; kullanıcı yönlendirmesi olmaksızın internet üzerinde hedef tarama/otomatik hedef seçimi yapma.

## 8. Performans ve kalite hedefleri

Bu değerler **ürün hedefidir, doğrulanmış performans vaadi değildir**:

- UI, normal test koşullarında akıcı ve etkileşimli kalmalı; network yükünü UI thread'i üretmemeli.
- En az 100 eşzamanlı HTTP sanal kullanıcı ile yerel örnek server üzerinde uçtan uca test geçmeli.
- Uygun koşullarda 1.000 kullanıcı konfigürasyonunu destekleyecek mimari kurulmalı; kapasite sonuçları donanım ve hedef servisle benchmark edilmeli.
- 15 dakikalık testte metric/log bellek büyümesi sınırlı olmalı.
- Stop işlemi bounded süre içinde testi durdurmalı; süre hedefi test senaryosundaki timeout'larla uyumlu tanımlanmalı.
- `cargo fmt --check`, `cargo clippy --workspace --all-targets` ve `cargo test --workspace` geçmeli (dış bağımlılık/ortam sorunları ayrı raporlanır).
- Mac üzerinde uygulamanın başlatılması ve temel demo akışının manuel doğrulanması.

## 9. Test stratejisi

**Unit tests**
- Test planı validation ve JSON round-trip
- Placeholder ve scoped variable çözümlemesi
- CSV satır döngüsü ve eksik alan davranışı
- Assertions (status/json/latency)
- Histogram ve percentile hesaplamaları
- Ramp-up hesaplaması, pacing/RPS limit, status sınıflandırması

**Integration tests (localhost fixture server)**
- GET 200 ve POST JSON
- 401/429/500 cevapları
- Timeout, disconnect, malformed JSON
- Login → extract token → authenticated request
- 100 user concurrent smoke, ramp-up, stop/cancel
- SQLite history write/read; HTML/JSON/CSV export

**Desktop/manual checks**
- Studio'da plan oluştur/değiştir/kaydet/yeniden aç
- Light/Dark geçişi ve kalıcılığı
- Monitor gerçek metrik/grafik güncellemesi
- Test sürerken UI'nin yanıt vermeye devam etmesi
- Geçersiz URL, gizli token redaction, yetki onayı
- Pencere küçültülünce panel davranışı

**Not:** Test amacıyla yalnız loopback, yerel fixture veya açıkça izin verilen staging hedefleri kullanılmalı.

## 10. Aşamalı geliştirme planı

### M0 — İskelet ve UI sözleşmesi

- Rust workspace + lint/test/format pipeline
- Semantic theme tokens + Dark/Light
- Studio/Monitor navigasyon iskeleti
- Core TestPlan ve RunConfig veri modeli
- Karar kayıtları ve README

**Çıkış kriteri:** Uygulama açılıyor, iki ekran var, tema değişiyor, Core modelleri round-trip testten geçiyor.

### M1 — Çalışan yerel HTTP engine

- Tek istek, ardından çoklu sıralı request desteği
- Async sanal kullanıcı yönetimi, ramp-up, duration, timeout, stop
- Global RPS limit ve güvenli defaults
- Canlı metric snapshot kanalı
- Yerel fixture server ile gerçek HTTP integration test

**Çıkış kriteri:** 100 VU localhost testi çalışıyor, Stop güvenilir, ölçümler gerçek request'lerden geliyor.

### M2 — Tam Studio editörü

- Test Plan Tree, Request Editor, headers/body/auth
- Variables, JSON extraction, CSV data, assertions
- Plan kaydet/aç, validation ve kritik UI durumları

**Çıkış kriteri:** Login → extract token → protected endpoint → assertion senaryosu sadece UI'dan kurulup çalıştırılıyor.

### M3 — Monitor ve raporlama

- KPI kartları, zaman serisi grafikleri, endpoint breakdown, logs
- SQLite test history
- HTML/JSON/CSV export
- UI responsiveness ve bounded memory doğrulaması

**Çıkış kriteri:** Gerçek test çalışırken Monitor güncelleniyor, bitince geçmiş ve rapor doğru görünüyor.

### M4 — Stabilizasyon ve dağıtım hazırlığı

- Integration/regression suite, güvenlik kontrolleri, redaction
- Kapanma/stop race condition testleri
- macOS packaging yaklaşımı ve manuel smoke test
- İleride Windows/Linux için belgelendirilmiş paketleme/CI planı

**Çıkış kriteri:** Aşağıdaki Definition of Done tamam.

## 11. Definition of Done — MVP teslim kabulü

- [ ] Rust + egui/eframe dışında frontend stack yok.
- [ ] İki ana ekran: Studio ve Monitor.
- [ ] Dark/Light temalar tasarım token'larıyla tutarlı.
- [ ] Studio'da çoklu HTTP request senaryosu gerçek HTTP istekleri gönderiyor.
- [ ] GET/POST/PUT/PATCH/DELETE, headers/body/query/auth çalışıyor.
- [ ] Token extraction, değişkenler, CSV ve assertions uçtan uca çalışıyor.
- [ ] Constant users, ramp-up, test duration, think time ve RPS üst sınırı uygulanıyor.
- [ ] Start/Stop ve timeout/cancellation doğru davranıyor.
- [ ] Monitor bütün metrikleri gerçek engine'den alıyor.
- [ ] Status/error ayrımı, P50/P90/P95/P99 ve endpoint bazlı sonuçlar doğrulanıyor.
- [ ] JSON planlar kaydedilip yeniden açılıyor.
- [ ] Geçmiş testler SQLite'da tutuluyor; HTML/JSON/CSV rapor çıkıyor.
- [ ] Sensitif değerler log ve raporda redakte ediliyor.
- [ ] Yerel fixture ile 100 VU end-to-end acceptance testi başarılı.
- [ ] `cargo fmt`, `cargo clippy`, `cargo test` kontrolleri başarılı; doğrulanamayanlar dürüstçe raporlu.
- [ ] README çalıştırma, mimari, bilinen limitler ve demo senaryosu içeriyor.

## 12. Demo / kabul senaryosu

**Hedef:** Yerel HTTP fixture server, gerçek internet hedefine yük gönderme yok.

**Plan:**
1. `POST /login` → JSON içinden `access_token` al.
2. `GET /products` → `Authorization: Bearer {{access_token}}` ile iste.
3. `POST /checkout` → CSV'den `product_id` kullan.
4. Status/assertion kontrollerini uygula.

**Yük profili:** 100 VU, 30 sn ramp-up, 2 dk duration, sabit think-time, güvenli RPS limiti.

**Beklenen:** Gerçek başarılı/hatalı request sayıları, percentiles, RPS ve VU grafikleri oluşur; test sonunda SQLite geçmiş ve dışa aktarılabilir rapor bulunur. Stop edildiğinde status Cancelled/Stopped olarak doğru yansır.

---

## 13. Codex teslim formatı

Uygulamayı oluşturduktan sonra şu biçimde rapor ver:

1. **Implemented:** Tamamlanan M0–M4 kilometre taşları ve ilgili dosyalar.
2. **Validation:** Gerçek çalıştırılan `cargo` komutları, sonuçları ve localhost senaryo kanıtları.
3. **Screenshots:** Mümkünse Studio Dark, Studio Light, Monitor Dark, Monitor Light ekran görüntüleri; mümkün değilse açıkça belirt.
4. **Known limitations:** Desteklenmeyen/eksik özellikler ve riskler.
5. **How to run:** macOS için adım adım çalıştırma; Windows/Linux hakkında yalnız doğrulanmış bilgi.
6. **Next steps:** MVP dışı özellikleri öncelik sırasına koy.

**Başarı ölçütü:** Ekranın referans görsele benzemesi tek başına yeterli değildir; **test motorunun güvenilir biçimde gerçek istekler üretmesi ve metriklerin doğru hesaplanması** birincil şarttır.
