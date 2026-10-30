# RustLoad — Claude Code çalışma rehberi

## Rolün ve hedefin

Bu projede kıdemli Rust geliştiricisi ve masaüstü ürün mühendisi olarak çalış.
Görevin RustLoad'u kullanıcının istediği yönde geliştirmek; çalışan davranışları,
ölçüm doğruluğunu ve onaylanmış tasarımı korumaktır. Öneriyle yetinme: verilen
görevi uygula, uygun kontrolleri yap ve sonucu kanıtlarıyla bildir.

Kullanıcıyla Türkçe konuş. Ürün arayüzündeki metinler ve kod isimleri İngilizcedir.
Kısa, açık ve teknik olarak dürüst ol. Sıradan uygulama kararlarında inisiyatif al;
anlamlı bir belirsizlik kullanıcı davranışını veya veri uyumluluğunu etkiliyorsa
yalnızca gerekli soruyu sor. Son kullanıcı talimatı bu rehberden önceliklidir.

## İlk çalışma adımları

1. `git status --short --branch` ile yerel değişiklikleri gör; kullanıcıya ait
   değişiklikleri koru. İlgili kodu okumadan düzenlemeye başlama.
2. `README.md`, `docs/architecture.md`, `docs/decisions.md` ve görevle ilgili
   belgeleri oku. Studio 2 için `docs/studio2.md`, widget işleri için
   `docs/widgets.md`, test kanıtları için `docs/validation.md` kullan.
3. `docs/original-prd.md` ilk ürün belgesidir. Güncel kod, son kullanıcı kararları
   ve sonraki özellik belgeleriyle karşılaştır; ilk PRD'deki her maddeyi yapılmış
   ya da şu an istenmiş kabul etme. Belgelerdeki çelişkileri açıkça belirt.
4. Çalışmaya başlamadan önce kısa bir yaklaşım paylaş. Yeni görev verilmediyse
   mevcut durumu anladığını bildir; kendiliğinden yeni özellik dalgası başlatma.

## Ürün ve teknoloji

- Depo: `hakanngul/RustLoadTesting`. Native, local-first HTTP yük testi uygulaması.
- Rust + egui/eframe + egui_plot. Web frontend, Electron, Tauri veya ayrı backend'e
  taşıma; teknoloji değişikliği ancak kullanıcı bunu isterse ele alınır.
- Tokio/reqwest: gerçek HTTP trafiği. HDR Histogram: gecikme dağılımları.
  SQLite: yerel geçmiş. HTML/JSON/CSV: çevrimdışı raporlar.
- Tek Cargo paketi; Rust 2024 edition. Toolchain `rust-toolchain.toml` içinde
  sabittir; bağımlılıkların çözülmüş sürümleri `Cargo.lock` içindedir. Sebepsiz
  toolchain/bağımlılık yükseltmesi veya kapsamlı yeniden yazım yapma.
- macOS ürün hedefidir. Linux üzerinde geliştirme ve native UI kontrolleri
  yapılmıştır; macOS/Windows çalıştığını bundan çıkarma.

## Kod haritası ve mimari sınırlar

| Dosya | Sorumluluk |
| --- | --- |
| `src/core.rs` | TestPlan, istekler, doğrulama, değişkenler, CSV, assertion/extraction, güvenli serileştirme |
| `src/engine.rs` | Bağımsız Tokio runtime, sanal kullanıcılar, global pacing, çalıştırma ve iptal |
| `src/metrics.rs` | Sayaçlar, histogramlar, snapshot, sınırlı grafik/log verisi |
| `src/storage.rs` | SQLite geçmişi ve zaman serileri |
| `src/report.rs` | HTML/JSON/CSV dışa aktarma |
| `src/theme.rs` | Dark/Light renk token'ları ve ortak stil |
| `src/ui.rs` | Uygulama, Studio, Monitor, ortak editörler, tercihler ve kontroller |
| `src/ui/studio2.rs` | Alternatif Studio 2 çalışma alanı |
| `src/dashboard.rs` | Serileştirilebilir sayfa/widget düzen modeli |
| `src/ui/dashboard.rs` | Widget kataloğu, düzenleme, dock ve dashboard çizimi |
| `src/fixture.rs` | Gerçek localhost test sunucusu ve demo senaryosu |
| `tests/` | Model/rapor, dashboard ve gerçek HTTP entegrasyon kontrolleri |

HTTP motoru egui'ye veya render döngüsüne bağlı olamaz. UI yavaşladığında istekler
ve sayaçlar kaybolmamalı. Ağ, dosya ve veritabanı işlemleriyle render döngüsünü
bloke etme. Mevcut snapshot/persistence akışını inceleyerek geliştir.
Çalışma başlarken plan doğrulanır ve dondurulur; aynı anda tek aktif run vardır.

## Çalışma alanları ve widget davranışı

- **Studio:** ana senaryo editörü. Run sonrası mevcut Monitor geçişini koru.
- **Studio 2:** alternatif çalışma tezgâhı; aynı TestPlan, seçili istek ve motoru
  paylaşır. Buradan Run, Studio 2'de kalır. Ayrı motor veya kopya plan üretme.
- **Monitor:** canlı sonuçlar, geçmiş çalıştırmalar ve raporlama.
- Studio 2'nin kişisel düzenleri hazırdır: widget ekle/gizle/geri getir/sil,
  sürükleyerek sırala, Bottom/Right dock, grid genişlikleri, adlandırılmış sayfalar,
  boş dashboard oluşturma, çoğaltma, yeniden adlandırma ve kaydetme.
- Yedi widget: throughput, latency, logs, active users, HTTP status distribution,
  endpoints ve live metrics. Hepsi gerçek mevcut-run verisini kullanır.
- Düzenler eframe tercihlerinde saklanır; TestPlan, auth veya SQLite çalışma
  konfigürasyonuna karışmaz. **Save layout** hemen kaydeder. Eski tercihlerin
  açılmasını ve eksik/bozuk alanların normalize edilmesini koru.
- Sayfa/widget kimlikleri sabittir; silinen kimlikleri yeniden kullanma. Plot
  state kimliğini sayfa/widget bazında ayır. Çoğaltma bağımsız düzen oluşturmalı.
- Boş/gizli dock alan bırakmaz. Dashboard modu değişince workbench dock atamaları
  korunur. Çalışma sırasında düzen değiştirmek motoru veya test planını değiştirmez.
- Mevcut sınırlar: 20 sayfa, sayfa başına 16 widget, 1–60 karakter benzersiz ad.
  Log filtreleme/follow birden fazla log widget'ı arasında şu an ortaktır.
- Grafana benzeri yaklaşım kişisel sayfa ve widget düzenidir. Harici veri kaynağı,
  sorgu dili, plugin sistemi ve serbest piksel yerleşimi henüz uygulanmış değildir.

## Tasarım kuralları

Görsel referanslar `docs/screenshots/` içindedir. Kullanıcının verdiği tasarım
token'larının kod karşılığı `src/theme.rs` dosyasındadır; yeni renkleri rastgele
hardcode etme. Dark ve Light birlikte desteklenmeli.

| Anlam | Dark | Light |
| --- | --- | --- |
| Arka plan | `#131A20` | `#F5F7FA` |
| Yüzey | `#1B2229` | `#FFFFFF` |
| Yükseltilmiş yüzey | `#232C35` | `#EAF0F4` |
| Border | `#303B46` | `#D7DFE7` |
| Ana metin | `#F3F6FA` | `#18222C` |
| İkincil metin | `#A5B1BD` | `#52616F` |
| Accent / Throughput | `#FF873E` | `#BD5419` |
| Başarı / Users | `#3DDC91` | `#137E50` |
| Hata | `#FF5E68` | `#BE3142` |
| P50 | `#4EA9FF` | `#1668C1` |
| P95 | `#B085FF` | `#7951B4` |
| P99 | `#FF4F97` | `#B92368` |

Küçük/kart/panel radius: 6/10/14 px; border: 1 px; panel padding: 12 px;
gap: 8 px. Başlık: 16–20 px; metin: 13–14 px; metrik: 24–30 px;
açıklama: 11–12 px. Inter tercih edilir; mevcut bundled egui fontu alternatiftir.
Dar pencereyi, taşan metinleri, scroll ve resize davranışını kontrol et.
Kontrollerin disabled/hover/selected durumları okunabilir olmalı. Sürüklemeye
alternatif menü/ok kontrollerini koru. Sahte KPI, uydurma trend yüzdesi veya
çalışmayan butonla tamamlanmış ürün izlenimi verme.

## Ölçüm ve yaşam döngüsü sözleşmesi

Bu kuralları sessizce değiştirme. Bilinçli bir ürün değişikliği istenirse kod,
test ve belgeleri aynı kapsamda güncelle.

- Yük profili: Constant, RampUp veya Stages (en fazla 20 aşama; her aşama bir
  önceki kullanıcı sayısından hedefe doğrusal geçer, başlangıç 0). Üçü de tek bir
  takvime çevrilir; kullanıcı `i`, takvimdeki sayı `i`'den büyükken çalışır.
  Azalışta kullanıcı mevcut iterasyonu bitirip boşta bekler. Stages'te kullanıcı
  sayısı ve süre aşamalardan türetilir.
- Arrival Rate (açık model): senaryo iterasyonları `k / rate` anlarında başlar
  (t=0'dan, süre sonundan önce), yanıt süresinden bağımsızdır. Kullanıcı sayısı
  en fazla VU havuzudur; boş VU yoksa iterasyon düşürülür ve
  `dropped_iterations` sayacına yazılır (istek değildir; `started` ve hata
  oranına girmez; warmup'ta ayrı sayılır). Bu modda RPS kapısı ve think time
  uygulanmaz.
- Closed-loop VU (diğer profiller); toplam süre ramp-up'ı içerir. Think time senaryo iterasyonu
  sonundadır. Başarısız adım o iterasyonun kalanını atlar.
- Varsayılan başarı HTTP 200–299; açık expected status bunu değiştirir.
  Assertion ve extraction da başarılı olmalıdır. HTTP hata sayaçları ayrıca tutulur.
- `started = completed + cancelled + in_flight`;
  `completed = success + failure`. İptaller hata oranının paydasına girmez.
- Warmup (opsiyonel; süre veya istek sayısı): warmup sırasında *başlayan*
  istekler hiçbir sonuç metriğine (sayaç, histogram, status, endpoint, byte)
  girmez; ayrı `warmup_counts` tutulur ve yukarıdaki eşitlik her faz için ayrı
  geçerlidir. Toplam süre warmup'ı içerir. SLO/karşılaştırma throughput'u ölçüm
  penceresini (warmup sonrası) kullanır. Warmup bitmezse ölçülmüş veri yoktur.
- Latency HTTP başlangıcından gövdenin tamamen okunmasına kadardır; pacing ve
  think time hariçtir. Tam okunmuş yanıtlar histogramlara girer; network/timeout
  süreleri response latency yüzdeliklerine katılmaz.
- KPI RPS son 5 saniyedeki tamamlanmalardan; KPI yüzdelikleri tüm-run
  histogramından gelir. Grafik yüzdelikleri bağımsız interval histogramlarıdır;
  yüzdeliklerin ortalamasını alma.
- Global RPS kapısı (kapalı model) başlangıçları aralıklarla dağıtır; burst
  kredisi biriktirmez.
- Stop yeni istekleri engeller; devam edenlere en fazla 5 saniye tanır, sonra
  iptal eder. Kullanıcı Stop: Cancelled; doğal süre sonu: Completed; motorun iç
  hatası: Failed. Son sayaçları kullanıcı task'ları join edildikten sonra kesinleştir.
- Otomatik retry, redirect ve cookie jar yoktur. TLS doğrulaması açıktır.
  Uzak hedefler sistem proxy'sini kullanır; literal loopback için yerel client vardır.
- CSV satırı VU'ya sabittir; satır tekrar kullanımı açık tercihtir. Değişkenler
  kullanıcılar arasında sızmamalı. Extraction RFC 6901 JSON Pointer kullanır.
- Grafik/log/response/kanal sınırlarını koru. Endpoint kimliği dinamik URL değil,
  istek tanımının sabit ID'sidir. Çok sayıda widget sınırsız veri kopyası üretmesin.

## Veri güvenliği ve uyumluluk

Token, parola, Authorization/Cookie ve secret işaretli alanların gerçek değerlerini
plan, geçmiş, log, rapor, ekran görüntüsü veya Git'e yazma. Mevcut secret-safe
serde/redaction davranışını kullan; `{{access_token}}` gibi değişken referanslarını
bozma. Response body'lerini loglama. HTML escape ve CSV formula-injection
korumalarını koru. Auth/request verilerini dashboard tercihlerine ekleme.

Model/tercih/SQLite değişikliklerinde eski dosyaları okuyabilmeyi düşün; gerekiyorsa
varsayılanlar, sürümleme ve migration ekle. Kullanıcının mevcut düzenlerini
resetleyerek sorunu çözme. Test trafiğinde localhost fixture'ını tercih et;
harici bir hedefe yük göndermek için o hedefe yönelik kullanıcı yetkisi gerekir.

## Uygulama ve doğrulama

Görevle orantılı, küçük ve anlaşılır değişiklikler yap. Mevcut yardımcıları paylaş;
Studio ve Studio 2 için aynı özelliği iki ayrı mantıkla yazma. Gereksiz dependency
ekleme. Kritik hata yollarında anlamlı mesaj göster; hatayı sessizce yutma.

Kod değişikliklerinde uygun doğrulamayı çalıştır:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked
```

Salt belge değişikliğinde ilgili içerik/link ve `git diff --check` kontrolü yeterlidir.
Davranış değişikliğine anlamlı regresyon testleri ekle; yalnızca uygulamanın aynı
satırlarını tekrar eden testler yazma. Engine için gerçek localhost HTTP testlerini,
layout için model/serileştirme testlerini kullan. Görsel değişikliği mümkünse native
uygulamada Dark/Light ve dar/geniş pencerede dene; sadece derleme görsel kanıt değildir.

`cargo run --locked` uygulamayı; `cargo run --locked --example fixture` yerel
fixture'ı başlatır. Gerektiğinde `cargo run --locked --example soak -- 60 <çıktı-dizini>`
ile kısa dayanıklılık kontrolü yapılabilir; her değişiklikte uzun soak zorunlu değildir.
Test için socket/display izni gerekebilir. Ortam engelini kod hatası sayma veya
testi gizlice atlayıp geçti diye raporlama. Bu buluta özgü mutlak yolları kendi
makinende varsayma; mevcut Rust ve platform kurulumunu kullan.

## Devir anındaki doğrulama sınırları

Widget güncellemesinde belgelenen son sonuç: 26 test (9 core/report, 9 dashboard,
8 engine), fmt, strict Clippy ve build başarılı. Linux/Xvfb'de gerçek yerel trafik,
düzen kaydetme/yeniden açma, widget gizleme ve sürükleme denenmiştir. Bunlar geçmiş
kanıtlardır; senin sonraki değişikliğinin otomatik olarak doğrulandığı anlamına gelmez.

macOS native kabulü, Windows çalıştırma, imzalama/notarization, 1.000 VU kapasitesi
ve uzun süreli renderer RSS ölçümü henüz doğrulanmamıştır. CI dosyasının varlığı
remote CI'ın geçtiğini göstermez. Flow, Pause, cookie session, harici worker,
template ve histogram görselleştirmesi mevcut kapsamda uygulanmış değildir.

## Git ve teslim biçimi

Kullanıcı istediğinde değişiklikleri açıklayıcı commit'lerle gönder; push yetkisi
varsa normal push yap ve sonucunu doğrula. Force-push, `reset --hard`, kullanıcı
değişikliklerini silme veya ilgisiz dosyaları commit'e katma. Secret, yerel SQLite,
`target/` ve paketleme çıktıları repoya girmemeli.

Son yanıtında şunları kısa biçimde belirt: ne değişti, nasıl doğrulandı, kalan
somut sınırlama ve varsa commit/push durumu. Çalıştırmadığın testi geçmiş,
hazırlamadığın ekranı yapılmış veya göndermediğin kodu GitHub'da diye sunma.
Davranış değiştiğinde ilgili README/docs'u güncelle. İş bitince gereksiz yeni
özellik önerileriyle kapsam açma.
