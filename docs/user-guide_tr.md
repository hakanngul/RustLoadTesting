# RustLoad kullanım kılavuzu

[English](user-guide.md) · [Türkçe](user-guide_tr.md)

Bu kılavuz, RustLoad'u açmış ve işini halletmek isteyen test uzmanları ve
geliştiriciler için yazıldı: senaryo kurmak, çalıştırmak, sonuçları okumak,
saklamak ve karşılaştırmak. Arayüz İngilizcedir; **kalın** yazılan ifadeler
uygulamada gördüğünüz metinlerin birebir aynısıdır.

İçindekiler

1. [RustLoad nedir?](#1-rustload-nedir)
2. [Kurulum ve ilk açılış](#2-kurulum-ve-ilk-açılış)
3. [Uygulamalı başlangıç: Local Demo ile ilk test](#3-uygulamalı-başlangıç-local-demo-ile-ilk-test)
4. [Senaryo oluşturma](#4-senaryo-oluşturma)
5. [Yük modelleri ve ayarlar](#5-yük-modelleri-ve-ayarlar)
6. [Çalıştırma, durdurma ve Monitor'ü okuma](#6-çalıştırma-durdurma-ve-monitorü-okuma)
7. [SLO eşikleri](#7-slo-eşikleri)
8. [Geçmiş, karşılaştırma ve raporlar](#8-geçmiş-karşılaştırma-ve-raporlar)
9. [cURL, HAR ve OpenAPI'den içe aktarma](#9-curl-har-ve-openapiden-içe-aktarma)
10. [Kişisel dashboardlar](#10-kişisel-dashboardlar)
11. [Tema, tercihler ve kısayollar](#11-tema-tercihler-ve-kısayollar)
12. [Sorun giderme ve SSS](#12-sorun-giderme-ve-sss)
13. [Metrik sözlüğü](#13-metrik-sözlüğü)

## 1. RustLoad nedir?

RustLoad, kendi bilgisayarınızdan bir API'ye gerçek HTTP trafiği gönderen ve
API'nin yük altında nasıl davrandığını ölçen bir masaüstü uygulamasıdır. Her
test bir **plandır**: sıralı HTTP isteklerinden oluşan bir senaryo ve bir yük
yapılandırması. Sanal kullanıcılar (VU) senaryoyu tekrar tekrar çalıştırır;
RustLoad da bu sırada latency, throughput, durum kodlarını ve hataları kaydeder.

Bir sürümden önce API'yi kontrol etmek, iki build'i ya da iki ayarı
karşılaştırmak veya latency'nin ve hataların hangi yükte artmaya başladığını
bulmak için kullanabilirsiniz. Her şey yerelde kalır: hesap, bulut servisi ya da
ayrı bir backend yoktur. Geçmiş yerel bir SQLite dosyasında tutulur, raporlar
çevrimdışı dosyalardır.

Bu bir 0.1 sürümü ve bilinçli sınırları var: istekler sabit bir sırayla çalışır
(dallanma veya döngü yok); otomatik yeniden deneme, yönlendirme takibi ve çerez
desteği bulunmaz. Tam liste için [Bilmeniz gereken sınırlar](#bilmeniz-gereken-sınırlar)
bölümüne bakın.

> **Yalnızca test etme yetkiniz olan sistemleri test edin.** Yük testi bir
> servisi yavaşlatabilir, hatta çökertebilir. RustLoad'u yalnızca size ait ya da
> test etmek için yazılı izin aldığınız sistemlere yöneltin. Her çalıştırmadan
> önce **I am authorized to test all targets in this plan.** (Studio) veya
> **I am authorized to test all configured targets.** (Studio 2) kutusunu
> işaretlemeniz gerekir. Planda bir şey değiştiğinde işaret kendiliğinden
> kalkar; böylece istek göndereceğiniz hedefleri her seferinde yeniden
> onaylarsınız. Planda geçen tüm origin'ler (şema, host ve port) kutunun altında
> listelenir.

## 2. Kurulum ve ilk açılış

### Hazır sürümü indirme

Bir sürüm yayımlandığında
[GitHub Releases](https://github.com/hakanngul/RustLoadTesting/releases)
sayfasında macOS DMG'leri (Apple Silicon ve Intel), Windows x64 ZIP'i, Linux x64
arşivi ve SHA-256 doğrulama dosyaları bulunur. Paketler Developer ID ile
imzalanmış ve notarize edilmiş değildir; macOS ve Windows ilk açılışta uyarı
gösterebilir. Ayrıntılar [release rehberinde](releases.md).

### Kaynak koddan derleme

1. Rust'ı [rustup](https://rustup.rs) ile kurun.
2. Depoyu klonlayıp çalıştırın:

   ```sh
   git clone https://github.com/hakanngul/RustLoadTesting.git
   cd RustLoadTesting
   cargo run --locked
   ```

   İlk derlemede `rust-toolchain.toml` içinde sabitlenen araç zinciri ve
   `Cargo.lock` içindeki bağımlılıklar indirilir.

OpenGL destekli bir masaüstü oturumu gerekir. Linux'ta X11/Wayland, xkbcommon ve
GL çalışma zamanı kütüphanelerini kurun. macOS'ta Xcode Command Line Tools'u
(`xcode-select --install`) kurun.

### Platform durumu

| Platform | Durum |
| --- | --- |
| Linux | Derleme, otomatik testler ve yerel arayüz kontrolleri yapıldı |
| macOS | Hedef platform; yerel kabul testleri henüz yapılmadı |
| Windows | Çalıştırma ve paketleme doğrulanmadı |

### Arayüzde ilk tur

Üst çubukta üç çalışma alanı düğmesi (**Studio**, **Studio 2**, **Monitor**) ve
**Report** bulunur. Hepsi aynı plan ve aynı çalıştırma üzerinde çalışır;
aralarında geçiş yapmak hiçbir testi başlatmaz ya da durdurmaz. Uygulama son
kullandığınızı hatırlar.

Hemen altında araç çubuğu yer alır: **+ New**, **Open**, **Save**, **Save as…**,
**Import…**, **Local Demo**, **▶ Run Test**, **■ Stop**, **Reset view**,
**Export Report** ve ardından düzen kontrolleri (**Layout: …**, **+ Widget**,
**Edit layout**, **Layouts…**, **Save layout**). Planda kaydedilmemiş değişiklik
varsa **Unsaved changes** yazısı görünür. Sağ üstte, **Local Engine** yazısının
altındaki durum satırı son mesajı (uzun mesajlar kısaltılır; tamamı için üzerine
gelin) ve RustLoad'un kendi kullandığı CPU ile RAM'i gösterir.

![Local Demo planı açık Studio; Login isteğinin Extractors sekmesi seçili](screenshots/guide/01-studio-demo.png)

- **Studio** ana düzenleyicidir. Solda **Test Plan**, onun altında
  **⚙ Users & plan settings** ve **HTTP Requests** listesi; ortada istek
  düzenleyici; sağda **Load Configuration** ve yetki onay kutusu bulunur.
  Grafikler en alttadır.
- **Studio 2** alternatif bir çalışma tezgâhıdır: daraltılabilir plan ağacı,
  kompakt istek düzenleyici, yan yana gövde ve yük ayarları, sağda canlı
  metrikler, altta grafikler ve loglar. Testi buradan başlatırsanız Studio 2'de
  kalırsınız.
- **Monitor** sonuçları gösterir: solda **Test Runs** listesi, sağda açık olan
  çalıştırma (canlı ya da geçmişten).
- **Report** aynı açık çalıştırmayı sabit, salt okunur bir özet olarak gösterir
  (bkz. [Report](#report)). Monitor'ü ve sayfalarını değiştirmez.

## 3. Uygulamalı başlangıç: Local Demo ile ilk test

Local Demo, RustLoad'un içinde rastgele bir `127.0.0.1` portunda küçük bir test
sunucusu açar ve ona yönelik üç adımlı bir senaryo yükler. Bilgisayarınızın
dışına hiç trafik çıkmaz.

1. Kaydedilmemiş değişikliği olmayan bir planla başlayın (önce kaydedin ya da
   değişiklikleri atın; **Unsaved changes** görünürken **Local Demo** pasiftir).
2. **Local Demo**'ya tıklayın. Üç istekli **Local API · Checkout scenario** planı
   açılır:
   - **Login** (POST `/login`), JSON yanıttaki `/access_token` değerini
     `access_token` değişkenine çıkarır.
   - **List products** (GET `/products`), **Auth** sekmesi üzerinden
     `Authorization: Bearer {{access_token}}` gönderir.
   - **Checkout** (POST `/checkout`), `{"product_id":"{{product_id}}"}` gövdesini
     gönderir ve `201` durum kodunu bekler (**Assertions** sekmesine bakın).
     `product_id`, **⚙ Users & plan settings** altındaki bir plan değişkenidir.
3. **Load Configuration**'a bakın: 10 sanal kullanıcı, 30 sn,
   **Constant Users**, **Global RPS cap** 20, **Think / iteration** 100 ms.
4. İsteğe bağlı: **⚙ Users & plan settings**'i açın, **SLO thresholds**
   bölümüne inip **+ Add threshold**'a tıklayın (varsayılan: P95 latency
   ≤ 300 ms). İlk saniyeleri sonuçların dışında tutmak için **Warmup**'ı
   **By duration** yapın.
5. **I am authorized to test all targets in this plan.** kutusunu işaretleyin.
   Altında yerel `http://127.0.0.1:<port>` adresi listelenir.
6. **▶ Run Test**'e tıklayın. RustLoad Monitor'e geçer ve grafikler dolmaya
   başlar.
7. Testin 30 saniyede bitmesini bekleyin (**Completed**) ya da erken bitirmek
   için **■ Stop**'a basın (**Cancelled**; o ana kadarki sonuçlar korunur).
8. **Export Report**'a tıklayın, **HTML**, **JSON** veya **CSV** seçin, isterseniz
   **File path** alanını değiştirin ve **Continue**'ya basın.

![Demo bittikten sonra Monitor: SLO sonucu, warmup satırı, KPI kartları, grafikler, durum kodları ve loglar](screenshots/guide/02-monitor-results.png)

Çalıştırma artık **Test Runs** listesinde; uygulamayı yeniden başlattığınızda da
orada kalır.

Demo sunucusu yalnızca uygulama açıkken yaşar. Demo planını kaydedip uygulamayı
yeniden başlattıktan sonra açarsanız eski port artık yoktur ve istekler
"Connection refused" hatasıyla düşer. Sabit portlu bir sunucu için bir terminalde
`cargo run --locked --example fixture` çalıştırın (Ctrl-C'ye kadar
`127.0.0.1:8080`'i dinler) ve istek URL'lerini bu adrese çevirin.

### k6 Demo (internet)

**k6 Demo**, Grafana k6'nın yük testi alıştırması için yayınladığı herkese açık
QuickPizza demo API'si (`https://quickpizza.grafana.com`) için hazır bir plan
yükler: Home, Quotes, Doughs ve Recommend pizza (JSON gövdeli POST). Her istek
200 durum kodu ve yanıt süresi sınırıyla kontrol edilir; Doughs ayrıca
`/doughs/0/name` değerinin `"Thin"` olduğunu doğrular, Recommend pizza
`/pizza/name` değerini çıkarır. Yük: Ramp-up Users, 10 kullanıcı, 10 sn
ramp-up, 10 sn warmup dahil toplam 70 sn, RPS sınırı 20, 1 sn think time.
SLO'lar: P95 ≤ 800 ms, P99 ≤ 1500 ms, hata oranı ≤ %1, throughput ≥ 10 req/s.
Demo API token'ı gizli `pizza_token` değişkeninde rastgele üretilir (QuickPizza
16 karakterlik her token'ı kabul eder); bu yüzden kaydedilmiş bir kopya token'ı
yeniden ister: `token ` ve ardından herhangi 16 harf veya rakam girin ya da
**k6 Demo**'ya yeniden tıklayın. Ortak kullanılan bir servis olduğu için yükü
makul tutun ve yetki kutusunu yalnızca kullanım koşullarını kabul ediyorsanız
işaretleyin.

## 4. Senaryo oluşturma

### İstekler

- **+ Add request** (Studio kenar çubuğunda ya da Studio 2 ağacında) listenin
  sonuna bir GET isteği ekler. Bir planda 1 ile 500 arası istek olabilir.
- Bir isteğe sağ tıklayarak **Duplicate**, **Move up**, **Move down** ve
  **Delete** seçeneklerine ulaşırsınız.
- Her iterasyonda istekler yukarıdan aşağıya çalışır. Bir adım başarısız olursa
  o iterasyonun kalanı atlanır ve sanal kullanıcı sonraki iterasyona geçer
  (kapalı modellerde think time'dan sonra).

Her istek için **Name**, **Method** (GET, POST, PUT, PATCH, DELETE) ve **URL**
alanlarını doldurun. URL'ler tam `http://` veya `https://` adresi olmalıdır.
Düzenleyicideki sekmeler:

| Sekme | Ne işe yarar |
| --- | --- |
| **Params** | **Name** / **Value** / **Secret** satırları olarak sorgu parametreleri |
| **Headers** | İstek header'ları; aynı tablo |
| **Body** | İstek gövdesi. **JSON body** işaretliyse `Content-Type: application/json` gönderilir. Studio 2'de gövdenin **JSON** / **Raw text** seçicili ayrı bir paneli vardır |
| **Auth** | **None**, **Basic** (**Username**, **Password (session-only)**) veya **Bearer** (**Token or {{variable}} (session-only)**) |
| **Assertions** | Beklenen durum kodu, azami yanıt süresi ve JSON değer kontrolü |
| **Extractors** | JSON yanıttan değişkenlere değer aktarma |
| **Advanced** | Bu istek için **Override request timeout** |

URL içinde kimlik bilgisi (`https://kullanici:parola@host`) kabul edilmez;
**Auth** sekmesini kullanın. Adı hassas görünen sorgu parametreleri (`token`,
`api_key` gibi) URL'den **Params**'a taşınmalı ve secret olarak işaretlenmelidir.

### Değişkenler ve `{{var}}` referansları

URL path'inde ya da sorgu değerinde, header'larda, gövdede veya auth
alanlarında `{{ad}}` yazabilirsiniz. Değerler üç kaynaktan gelir:

1. **⚙ Users & plan settings** (Studio) veya Studio 2 ağacındaki
   **CSV Data / Variables** altındaki **plan değişkenleri**.
2. **CSV sütunları** (aşağıya bakın). Aynı adlı plan değişkenini ezerler.
3. **Extractor'lar**; değişkeni yalnızca o sanal kullanıcı için günceller.

Her sanal kullanıcı değişkenlerin kendi kopyasıyla çalışır; değerler
kullanıcılar arasında taşınmaz. URL'nin şeması ve host'u sabit yazılmalıdır:
`https://{{host}}/x` reddedilir, böylece veri yüzünden test başka bir sunucuya
yönelemez. İstek hazırlanırken değeri olmayan bir değişken, o adımı yerel
hazırlık hatasıyla ("Missing variable: …") başarısız sayar.

### Veri çıkarma ve istekleri zincirleme

**Extractors** sekmesinde **+ Add extractor**'a tıklayıp şunları doldurun:

- **Variable**: değerin yazılacağı ad, örneğin `access_token`.
- **/json/pointer**: `/access_token` ya da `/items/0/id` gibi bir RFC 6901 JSON
  Pointer. JSONPath değil, JSON Pointer kullanılır.

Metin değerler olduğu gibi, sayı, boolean ve nesneler JSON metni olarak saklanır.
Pointer bir şey bulamazsa istek başarısız sayılır ("Extraction found nothing at
…") ve iterasyon orada biter. Değişkeni sonraki isteklerde kullanın; örneğin
**Auth** → **Bearer** → `{{access_token}}`.

### Assertion'lar ve beklenen durum kodu

Varsayılan olarak bir istek; durum kodu 200–299 ise, gövde tamamen okunmuşsa ve
tüm assertion ile extraction'lar geçmişse başarılıdır. **Assertions**
sekmesinde şunları yapabilirsiniz:

- **Exact expected status (otherwise 200–299)** kutusunu işaretleyip tek bir kod
  girmek (örneğin `201`, negatif bir test için `401`);
- **Maximum response time** kutusunu işaretleyip ms cinsinden bir sınır girmek;
- **JSON Pointer (optional, e.g. /created)** ve **Expected JSON value (e.g. true,
  123, or "text")** alanlarını doldurmak. Beklenen değer geçerli JSON olmalı;
  metinler tırnak içinde yazılır.

Beklenen `401` dönen bir istek başarılı sayılır, ama 401 yine de HTTP durum kodu
sayımlarında görünür.

### Sanal kullanıcı başına CSV verisi

1. Başlık satırı olan bir CSV dosyasını kaydettiğiniz planın yanına koyun.
2. **⚙ Users & plan settings** → **CSV data source** altında yolu
   **Path (relative to saved plan directory)** alanına yazın. Kaydedilmemiş bir
   planda yol, RustLoad'un başlatıldığı klasöre göre çözülür.
3. Başlıklar değişken olur (`{{email}}`, `{{password}}`, …).

Birinci sanal kullanıcı ilk veri satırını, ikinci kullanıcı ikinci satırı alır
ve bu böyle sürer; her kullanıcı test boyunca kendi satırını korur. Satır sayısı kullanıcı
sayısından azsa plan reddedilir. **Reuse rows when there are more users than
data rows** işaretlenirse satırlar başa dönerek tekrar kullanılır. Başlıklar
benzersiz ve boş olmayan değerler olmalı, dosya en fazla 16 MiB olabilir.

### Secret'lar: neler kaydedilir, neler kaydedilmez?

RustLoad kimlik bilgisi değerlerini plan dosyalarına, geçmişe, loglara veya
raporlara asla yazmaz.

- Bir parametreyi, header'ı ya da plan değişkenini kimlik bilgisi olarak işlemek
  için **Secret** kutusunu işaretleyin. Adında `authorization`, `cookie`,
  `password`, `passwd`, `token`, `api_key` veya `secret` geçen alanlar her zaman
  secret'tır (kutu kilitlidir) ve değerleri maskelenir.
- **Basic** parolaları ve **Bearer** token'ları her zaman secret'tır.
- Değeri tam olarak tek bir değişken referansı olan secret'lar, örneğin
  `{{access_token}}`, bu referansla kaydedilir. Düz değerler ve
  `Bearer {{token}}` gibi karışık metinler kaydederken düşürülür.
- İstek gövdeleri, **Persist body in plan (ensure it contains no credentials)**
  (Studio) veya **Save body in plan (no literal secrets)** (Studio 2)
  işaretlenmedikçe yalnızca oturumda kalır. Kimlik bilgilerini secret
  değişkenlere koyun ve gövdeden onlara referans verin.
- Geçmiş kayıtları istek gövdelerini ve URL sorgu metinlerini içermez. Yanıt
  gövdeleri hiçbir zaman loglanmaz ya da saklanmaz.

Bir planı **Open** ile açtıktan sonra, çalıştırmadan önce atlanan secret
değerleri ve oturuma özel gövdeleri yeniden girin. Eksik olanı doğrulama mesajı
söyler; örneğin "Re-enter secret value: api_key" ya da "Re-enter Bearer token or
variable reference".

### Planları kaydetme ve açma

- **Save** geçerli dosyaya yazar; dosya yoksa bir yol sorar (varsayılan
  `rustload-plan.json`). **Save as…** her zaman sorar. Var olan bir yola
  kaydetmek o dosyanın yerine yazar. **+ New** ve **Local Demo** geçerli yolu
  temizler; sonraki **Save** yeni bir yol sorar.
- **Open** bir dosya yolu ister. En fazla 16 MiB boyutunda, şema sürümü 1 olan
  planlar açılabilir.
- **+ New** boş bir plan açar. Kaydedilmemiş değişiklik varsa **Unsaved plan**
  penceresi **Keep editing** veya **Discard changes** seçeneklerini sunar.
- **Validate plan** (**⚙ Users & plan settings**'in en altında) trafik
  göndermeden planı ve CSV dosyasını kontrol eder. Sonuç butonun altında yeşil
  **Plan is valid** veya kırmızı **Plan has a problem** kartı olarak görünür
  (mesaj isteğin adını verir, örneğin `Request "Pi": Invalid URL`) ve plan
  değişene kadar kalır.

Dosya pencereleri sistem dosya seçicisi değil, düz yol alanlarıdır. Göreli
yollar uygulamanın başlatıldığı klasöre göre çözülür.

## 5. Yük modelleri ve ayarlar

Yük ayarları **Load Configuration** bölümündedir (Studio'da sağ panel, Studio
2'de sağ alt). Bir test çalışırken kilitlidir.

### Profiller

**Profile** (Studio) veya **Load profile** (Studio 2) altından birini seçin:

| Profil | Yük nasıl üretilir |
| --- | --- |
| **Constant Users** | Tüm **Virtual users** aynı anda başlar ve **Total duration** boyunca senaryoyu döngüyle çalıştırır |
| **Ramp-up Users** | Kullanıcılar **Ramp-up** süresine (Studio 2'de **Ramp-up time**) eşit aralıklarla yayılarak başlar, sonra sabit kalır. Ramp-up toplam sürenin içindedir |
| **Stages** | Kullanıcı sayısı en fazla 20 aşamayı izler; her aşama bir önceki sayıdan (başta 0) kendi **Target users** değerine, kendi **Duration** süresi içinde doğrusal olarak geçer |
| **Arrival Rate (open)** | İterasyonlar, hedefin ne kadar hızlı yanıt verdiğinden bağımsız olarak sabit bir hızla (it/s) başlar; en fazla **Max VUs** kadar kullanıcılık bir havuz kullanılır |

İlk üçü **kapalı döngü** (closed-loop) modellerdir: her kullanıcı kendi yanıtını,
ardından **Think / iteration** süresini bekler ve sonra yeniden başlar. Hedef
yavaşladığında uygulanan yük de onunla birlikte düşer.

**Stages** bir tablo ve önizleme grafiği gösterir. **Duration** ve
**Target users** değerlerini tablodan düzenleyin, **+ Add stage** ya da **×** ile
aşama ekleyip silin veya grafikteki noktaları sürükleyin (sağa-sola süreyi,
yukarı-aşağı değeri değiştirir). **Virtual users** ve **Total duration**
aşamalardan hesaplanır ve gri görünür. Sayı azaldığında kullanıcılar o anki
iterasyonu bitirip boşta bekler.

### Arrival Rate (açık model)

1. **Arrival Rate (open)** seçin. **Virtual users** alanı **Max VUs** olur.
2. **Arrival rate** değerini saniyede iterasyon olarak girin (0,01–100.000 it/s).
   Bir iterasyon, tüm isteklerin bir kez çalışmasıdır; dolayısıyla istek/sn
   değeri it/s'nin katı olabilir.
3. **Max VUs** değerini hız × iterasyon süresini karşılayacak, biraz da pay
   bırakacak kadar yüksek tutun.

Bir iterasyonun başlama zamanı geldiğinde tüm VU'lar meşgulse o iterasyon
başlatılmaz ve **dropped iteration** (düşürülen iterasyon) olarak sayılır.
Düşürülen iterasyonlar istek değildir; hata oranını değiştirmezler, ama
uygulanan yükün planlanandan düşük kaldığını gösterirler. Durum panelinde uyarı
rengiyle görünürler; görürseniz **Max VUs**'u artırın.

Hızı zamanla değiştirmek için **Rate stages** satırındaki **Ramp the rate**
kutusunu işaretleyin. Hız alanı **Start rate** olur ve **Target rate** sütunlu
bir aşama tablosu ile sürüklenebilir bir önizleme açılır. RustLoad başlangıç
için üç aşama oluşturur (artış, sabit, azalış). Her aşama bir önceki hızdan
kendi hedefine doğrusal olarak geçer; toplam süre aşamaların toplamıdır. Hızı
0 olan bir aşama yeni başlangıçları durdurur. En az bir aşamanın hızı 0'dan
büyük olmalıdır. Grafikler planlanan hızı kesikli bir **Target it/s** çizgisiyle
gösterir. Sabit hıza dönmek için kutunun işaretini kaldırın.

![Ramp the rate açık Arrival Rate profili: Start rate, hız aşamaları tablosu ve sürüklenebilir önizleme](screenshots/guide/04-arrival-rate-stages.png)

Bu modda **Global RPS cap** ve **Think / iteration** kullanılmaz ve pasif
görünür.

### Diğer ayarlar

| Ayar | Anlamı |
| --- | --- |
| **Total duration** | Ramp-up ve warmup dahil testin toplam süresi (1–86.400 sn) |
| **Global RPS cap** | Yalnızca kapalı modeller: tüm kullanıcılar için saniyede en fazla HTTP istek başlangıcı (1–100.000). Başlangıçlar eşit aralıklarla dağıtılır; boşta geçen süre biriktirilip toplu istek olarak harcanmaz |
| **Think / iteration** | Yalnızca kapalı modeller: her senaryo iterasyonundan sonraki bekleme (0–300.000 ms) |
| **Request timeout** | İstek başına zaman aşımı (1–300.000 ms). Tek bir istek bunu **Advanced** sekmesinden değiştirebilir |
| **Body cap** | Okunacak en büyük yanıt gövdesi, byte (varsayılan 10 MiB, en fazla 100 MiB). Daha büyük yanıt isteği başarısız yapar |
| **Warmup** | **None**, **By duration** (ilk N saniye) veya **By requests** (ilk N istek) |

Studio 2'de **Think / iteration**, **Request timeout** ve **Body cap**,
daraltılabilir **Timeout, think time & body limit** bölümündedir.

**Warmup** süresince *başlayan* istekler hiçbir sonuç metriğine (sayaçlar,
yüzdelikler, durum kodları, endpoint'ler, byte'lar) girmez. Ayrı sayılırlar ve
istek hızı grafiklerinde gri bir **Warmup** çizgisi ile **Warmup end**
işaretiyle gösterilirler. Warmup toplam sürenin içindedir ve ondan kısa
olmalıdır. Warmup hiç bitmezse (örneğin kısa bir testte istek sayısına göre
warmup) ölçülmüş veri oluşmaz.

100'den fazla kullanıcıda ya da 100'ün üzerindeki bir RPS sınırında (veya tepe
arrival rate'te) **Elevated load: verify target and local capacity.** uyarısı
çıkar. Bu bir hatırlatmadır, engel değildir.

### Hangi modeli seçmeliyim?

| Amaç | Seçim |
| --- | --- |
| Duman testi, hızlı kontrol | Az kullanıcı ve kısa süreyle **Constant Users** |
| N eşzamanlı kullanıcının ne yaşadığını görmek | **Constant Users** veya **Ramp-up Users** |
| Artan, sabitlenen ve azalan yük (soak, kademeli, ani artış) | **Stages** |
| Hedef yavaşlasa bile sabit kalması gereken istek hızı | **Arrival Rate (open)**; hız rampası için aşamalarla |
| Kırılgan bir hedefi ani yüklenmelerden korumak | **Global RPS cap** ile herhangi bir kapalı model |

Kapalı döngü testlerde hedef aşırı yüklendiğinde latency olduğundan iyi
görünebilir; çünkü yavaş yanıtlar yeni istekleri de yavaşlatır (coordinated
omission). Uygulanan yükün sabit kalması gerekiyorsa Arrival Rate kullanın.

## 6. Çalıştırma, durdurma ve Monitor'ü okuma

### Başlatma ve durdurma

1. Durum satırının bildirdiği sorunları giderin. **▶ Run Test** önce planı
   doğrular; bir sorun varsa "Validation: …" mesajı gösterir. Dikkat isteyen
   durum satırı mesajları renklidir: hata kırmızı, uyarı (örneğin işaretlenmemiş
   yetki kutusu) sarı, başarı yeşil. Test bitince durum satırı sonucu, tamamlanan
   ve başarısız istek sayılarını ve SLO kararını gösterir.
2. Yetki kutusunu işaretleyip **▶ Run Test**'e tıklayın. Studio'dan başlatırsanız
   Monitor'e geçilir; Studio 2'den başlatırsanız Studio 2'de kalırsınız.
3. Test sürerken plan ve yük ayarları kilitlidir ("The running configuration is
   locked. Stop the test to edit.").
4. **■ Stop** yeni istekleri hemen durdurur, think time ve pacing beklemelerini
   keser, devam eden isteklere iptal edilmeden önce en fazla 5 sn tanır. Test
   **Cancelled** olarak kaydedilir. Süresini dolduran test **Completed**, motorun
   iç hatasıyla biten test **Failed** olur.

Aynı anda yalnızca bir test çalışabilir. Test sırasında çalışma alanı ya da düzen
değiştirmek teste dokunmaz. **Reset view** ekrandaki sonucu temizler (Studio
2'de mevcut çalıştırmanın widget'larını da temizler); geçmiş etkilenmez.

### Monitor'ün yapısı

Başlıkta çalıştırmanın adı, durumu ve geçen süre, ardından değerlerin nasıl
ölçüldüğüne dair bir not bulunur: "RPS and data/sec: last 5s · Percentiles:
entire run, fully read responses · Latency in ms". Altında, gerekiyorsa:

- bir uyarı satırı (örneğin geçmiş kaydı yazılamadığında);
- warmup satırı ("Warmup in progress …", sonra "Warmup (first 3s) excluded 60
  requests · measured from 3.0s");
- SLO satırı (bkz. [SLO eşikleri](#7-slo-eşikleri)).

Bunların altındaki her şey yeniden düzenleyebileceğiniz bir widget sayfasıdır
(bkz. [Kişisel dashboardlar](#10-kişisel-dashboardlar)). Varsayılan sayfada
şunlar vardır:

**KPI kartları**

| Kart | Anlamı |
| --- | --- |
| **Virtual Users** | Aktif kullanıcı / hedef kullanıcı |
| **Requests / sec** | Son 5 saniyede saniye başına tamamlanan istek |
| **Data / sec** | Son 5 saniyede saniye başına yanıt gövdesi byte'ı (header'lar hariç) |
| **P50 / P95 / P99 Latency** | O ana kadar ölçülen ve tamamen okunan tüm yanıtların yüzdelikleri |
| **Error Rate** | Başarısız ÷ tamamlanan istek, % |
| **Request Count** | Tamamlanan istek sayısı (yalnızca ölçüm penceresi) |

**Grafikler** (x ekseni: geçen saniye; sürükleyerek kaydırın, tekerlekle
yakınlaştırın)

Her grafik serilerini başlığın yanında listeler; bir seriyi gizlemek ya da
göstermek için adına veya noktasına tıklayın. Kaydırdıktan ya da
yakınlaştırdıktan sonra başlık satırında **Reset zoom** belirir ve tüm testi
yeniden gösterir (grafiğe çift tıklamak da aynı işi yapar).

- **Requests per Second**: aralık başına **Total**, **Success** ve **Failed**
  tamamlanma; ilgiliyse **Warmup** ve **Target it/s** çizgileri.
- **Response Time (ms) · interval percentiles**: her aralık için ayrı hesaplanan
  P50, P95 ve P99. Bunlar ortalaması alınarak tüm-test değerlerine dönüştürülmez;
  burada görülen bir sıçrama kartlarda görünmeyebilir, tersi de olabilir.
- **Active Virtual Users**: zaman içinde çalışan kullanıcı sayısı.

**HTTP Status Distribution** her durum kodunu payıyla listeler. Altında şu
sayaçlar bulunur: **Network**, **Timeout**, **Body limit**, **Local
preparation**, **Unexpected status**, **Assertion/extraction**, **Cancelled** ve
**In-flight**. Arrival Rate testlerinde hız ve **Dropped iterations** da
eklenir. **Failure reasons · measured, most frequent first** başlığı altında
istek başına en sık beş hata nedeni görünür: "HTTP 500, expected 2xx",
"Timeout", "Connection refused", "Slower than the 500 ms limit" veya "JSON at
/ok did not match" gibi. Fareyle üzerine gelince ilk ve son görülme zamanı
çıkar. Tam liste (en fazla 100 grup) HTML ve JSON raporlarındadır. Nedenler
yalnızca durum kodlarından, plan değerlerinden ve sabit etiketlerden oluşur;
yanıt içeriği ve URL içerebilen ham ağ hata metni asla gösterilmez.

**Endpoint Breakdown · slowest P95 first** her istek tanımı için bir satır
gösterir: **Completed**, **Errors**, **Avg RPS**, **Data** ve **P50**–**P99**.
Satırlar genişletilmiş URL'ye göre değil, isteğin kendisine göre gruplanır.

**Live Logs** her başarısız isteği ve yaklaşık her yüz başarılı istekten birini
seviye, istek numarası, durum kodu ve latency ile gösterir. **All levels** /
**INFO** / **ERROR** ya da **Filter logs…** ile süzün; otomatik kaydırmayı
durdurmak için **Follow**'un işaretini kaldırın. En fazla 5.000 kayıt tutulur;
loglarda istek gövdesi veya kimlik bilgisi değeri bulunmaz.

**Latency Distribution** widget'ı (varsayılan sayfada yok; **+ Widget** ile
ekleyin) tüm-test histogramını, P50/P95/P99 işaretleriyle birlikte en fazla 40
logaritmik çubuk olarak çizer. Bir çubuğun aralığını ve sayısını görmek için
üzerine gelin.

Dar pencerelerde sağ dock **Dashboard widgets** bölümüne katlanır, alt dock ise
widget'larını sekmeler halinde gösterir.

### Test sırasında Studio 2

![Test sırasında Studio 2: plan ağacı, kompakt istek düzenleyici, yük paneli, Live Metrics ve alttaki grafikler](screenshots/guide/03-studio2-run.png)

Sağdaki **Live Metrics** widget'ı **Virtual Users**, **Requests / sec** (son 5
sn ve veri hızı), **P95 Latency** ile **Error Rate** (tüm test) ve **Average
Response Time** kartlarını, her birinde gerçek aralık verisinden çizilmiş küçük
bir eğilim çizgisiyle gösterir; altında tamamlanan ve başarısız istek sayıları
yer alır. Ağaçtaki **Summary Report** mevcut çalıştırmayı Monitor'de açar.

### Report

**Report** (üst çubukta Monitor'den sonra), Monitor'de açık olan çalıştırma için
önce "geçti mi?", sonra "sorun nerede?" sorusunu yanıtlar; başka bir çalıştırma
için soldaki **Test Runs** listesinden seçin. Düzeni sabittir ve Monitor
sayfalarını değiştirmez. Studio'dan başlatılan test yine Monitor'ü açar.

![Dört SLO eşiği ve 5 sn warmup ile demo çalıştırmasından sonra Report: PASSED sonuç bandı, SLO kutucukları, ana kartlar, warmup bandı gölgeli zaman çizelgesi, hatalar, HTTP durumu ve en yavaş istek](screenshots/guide/07-report.png)

- **Sonuç bandı**: SLO eşiklerine göre **PASSED** / **FAILED** /
  **INCONCLUSIVE**; eşik yoksa **COMPLETED**, **COMPLETED WITH ERRORS**,
  **CANCELLED** ya da **RUNNING**. Kullanıcı, profil, süre, istek ve hata
  sayıları ile warmup; her eşik için Pass/Fail ve ölçülen değeri gösteren bir
  kutucuk.
- **Dört kart**: Throughput (son 5 sn ve ölçüm penceresi ortalaması), P95 (tüm
  istekler P95 SLO'sunun ne kadarını kullandığını gösteren çubukla), Hata oranı
  (0'da yeşil, üstünde kırmızı) ve Sanal kullanıcı (test bitince ulaşılan tepe
  değer). P50, P99, veri hızı, istek sayısı, warmup'ta hariç tutulanlar ve
  düşürülen iterasyonlar alttaki şeritte yer alır.
- **Zaman çizelgesi**: İstek/sn, yanıt süresi ve kullanıcı grafikleri aynı zaman
  eksenini paylaşır. Birinde yakınlaştırma ya da imleç diğerlerini de taşır;
  warmup gri bir banttır. Tüm istekler için P95 SLO'su (yoksa P99) yanıt süresi
  grafiğinde kesikli çizgidir; veriden çok yukarıdaysa çizilmez, bir notla
  belirtilir. Yakınlaştırma ve gizlenen seriler çalıştırma başına tutulur.
- **Why did requests fail?** (hata yoksa "No failures", varsa en sık nedenler),
  **HTTP status** ve P95'e göre **en yavaş istek**.
- **Endpoints**: sıralamak için sütun başlığına tıklayın (üçgen yönü gösterir);
  çubuk her P95'i en yavaş istekle karşılaştırır.
- **Live Logs** tek satıra katlanmış olarak tutulan INFO/ERROR sayılarıyla
  durur; **Show logs** açar, **Errors only** yalnızca hatalarla açar.

## 7. SLO eşikleri

Eşikler bir çalıştırmaya geçti/kaldı sonucu verir.

1. **⚙ Users & plan settings**'i (Studio) açın ya da Studio 2 ağacında **SLO
   Thresholds** altındaki bir öğeye tıklayın (hiç eşik yoksa **Add SLO
   thresholds**).
2. **+ Add threshold**'a tıklayın. Bir **Metric** (**P50 latency**, **P90
   latency**, **P95 latency**, **P99 latency**, **Mean latency**, **Error
   rate**, **Throughput**), bir **Scope** (**All requests** ya da tek bir istek)
   ve bir **Target** seçin.
3. Latency (ms) ve hata oranı (%) üst sınırdır (≤); throughput (req/s) alt
   sınırdır (≥). Hedefe tam eşit bir değer geçer.

Bir planda en fazla 50 eşik olabilir. Eşikler testi asla durdurmaz. Test
sürerken satırda sonuçlarla birlikte "SLO currently passing" ya da "currently
failing" yazar; test bitince sonuç gösterilir:

- **Passed**: tüm eşikler karşılandı.
- **Failed**: en az biri karşılanmadı.
- **Inconclusive**: bir eşik için veri yok (örneğin hiç tamamlanmayan bir istek).

Latency eşikleri tüm-test histogramlarını, hata oranı tamamlanan istekleri
kullanır; throughput ise 5 saniyelik kart değeri değil, tamamlanan isteklerin
warmup sonrası ölçüm süresine bölümüdür. İptal edilen ya da başarısız olan testler kısmi olarak
işaretlenir. Sonuç geçmişe kaydedilir ve tüm rapor biçimlerinde yer alır.

**Import…** penceresinde **Replace current requests** seçerseniz tek bir isteğe
bağlı eşikler silinir; çünkü yeni istekler eski kimlikleri yeniden kullanır.

## 8. Geçmiş, karşılaştırma ve raporlar

### Geçmiş

Biten her test (tamamlanan, iptal edilen ya da başarısız olan) yerel geçmişe
kaydedilir ve Monitor'deki **Test Runs** listesinde en yenisi üstte olacak
şekilde adı, başlangıç zamanı (UTC), durumu, süresi ve istek sayısıyla görünür.

- **Current run** bu oturumdaki çalıştırmaya döner.
- **Open summary** kayıtlı bir çalıştırmayı Monitor'e yükler. Bütün widget'lar
  artık o çalıştırmayı gösterir ve **Export Report** onu dışa aktarır.
- **↻** listeyi yeniler.
- **Open data folder** `history.sqlite3` dosyasının bulunduğu klasörü dosya
  yöneticisinde açar. Sistemde bunu yapacak bir araç yoksa durum satırı bunu
  söyler ve klasörün yolunu gösterir.
- **Export history…** bir dosya adı sorar (varsayılan
  `rustload-history.sqlite3`) ve kayıtlı bütün çalıştırmaların tutarlı bir
  SQLite kopyasını arka planda yazar. Var olan bir dosyayı kabul etmez; başarı
  ya da hata durum satırında görünür.

Uygulamada silme düğmesi yoktur; bkz.
[Verilerim nerede saklanıyor?](#verilerim-nerede-saklanıyor)

### İki çalıştırmayı karşılaştırma

1. Karşılaştırmada referans almak istediğiniz çalıştırmada **Baseline**'a
   tıklayın. "Baseline pinned · choose Compare on another run" yazısı çıkar.
2. Başka bir çalıştırmada (aday) **Compare**'e tıklayın.

**Run comparison** ekranında şunlar bulunur:

- genel sonuç: **Improved**, **Regressed**, **Mixed**, **Unchanged** veya
  **No data**;
- baseline ile adayın yerini değiştiren **Swap** ve **Close comparison**;
- **Noise band ±** (varsayılan %2): bu bandın içindeki değişimler değişmemiş
  sayılır. Bu bir tolerans, istatistiksel anlamlılık testi değildir. Değer
  hatırlanır;
- kullanıcı sayısı, süre, profil, RPS sınırı, think time, timeout, warmup ya da
  istek adımları farklıysa, bir test normal bitmemişse veya arrival-rate
  iterasyonu düşürmüşse **Not a like-for-like comparison:** uyarıları.
  Karşılaştırma yine de gösterilir;
- throughput, P50/P90/P95/P99, ortalama latency, hata oranı ve veri hızı için bir
  **Whole run** tablosu (**Metric**, **Baseline**, **Candidate**, **Change**,
  **Result**). Veri hızı bilgi amaçlıdır (**Changed**) ve genel sonucu
  etkilemez;
- üst üste çizilmiş **Throughput (requests per second)** ve **P95 latency (ms) ·
  interval** grafikleri;
- istek kimliğine göre eşleştirilmiş **Per request · change from baseline (hover
  for values)** tablosu; yalnızca bir çalıştırmada bulunan istekler altında
  listelenir.

Karşılaştırmadaki throughput ve veri değerleri 5 saniyelik kart değeri değil,
ölçüm penceresinin ortalamasıdır (tamamlanan ÷ warmup sonrası geçen süre).

### Rapor dışa aktarma

**Export Report**, ekranda gösterilen test bittiğinde etkinleşir. Biçimi seçin,
**File path**'i kontrol edin ve **Continue**'ya basın. O yolda dosya varsa
üzerine yazılır.

| Biçim | İçerik |
| --- | --- |
| **HTML** | Çevrimdışı, tek dosyalık özet: yapılandırma, metrikler, SLO sonuçları, hata nedenleri, warmup ve arrival-rate bilgisi, zaman serileri. Etkileşimli değildir |
| **JSON** | Tam özet, secret'ları temizlenmiş yapılandırma ve aralık bazlı zaman serileri; metrik tanımlarıyla birlikte |
| **CSV** | İstek başına bir satır: sayılar, P50–P99, byte'lar ve SLO özeti |

Dışa aktarma, secret ve gövde içermeyen temizlenmiş planı kullanır. Metinler
HTML için kaçışlanır; CSV'de tablo programlarının formül sanabileceği hücreler
etkisiz hale getirilir. Zaman serisinin bir kısmı eksikse rapor bunu belirtir;
özet sayaçlar esas alınır.

## 9. cURL, HAR ve OpenAPI'den içe aktarma

**Import…**'a tıklayınca **Import requests** penceresi açılır. Bir kaynak seçin,
**Preview**'a basın, istekleri ve uyarıları inceleyin; **Add to current
requests** ya da **Replace current requests** seçip **Import**'a tıklayın. İçe
aktarma hiçbir ağ bağlantısı kurmaz; yalnızca yapıştırdığınız metni ya da
belirttiğiniz dosyaları okur.

İçe aktardıktan sonra yeni istekleri kontrol edin, gereken secret değişkenleri
doldurun ve yetki kutusunu yeniden işaretleyin.

### Copy as cURL

Tarayıcının DevTools'unda bir isteğe sağ tıklayıp *Copy as cURL (bash)* (ya da
*Copy all as cURL*) seçin ve bir veya birden fazla komutu yapıştırın.

- Sorgu metni **Params**'a taşınır; URL'deki `kullanici:parola@` Basic Auth olur.
- `-H`, `-X`, `-d`/`--data-raw`/`--data-binary`/`--data-urlencode`, `--json`,
  `-u`, `-G`, `-b`, `-A`, `-e` ve `--url` tanınır. JSON content type'lı bir JSON
  gövde, JSON gövde olarak aktarılır.
- Bağlantı header'ları, `Content-Length` ve `Accept-Encoding` kaldırılır; böylece
  yanıtlar sıkıştırmasız okunur ve boyutlar doğru kalır.
- `-k` ve `-L` bir notla yok sayılır (TLS her zaman doğrulanır, yönlendirmeler
  takip edilmez). `-F`, `-T` ve `@dosya` verisi atlanır; HEAD/OPTIONS istekleri
  aktarılmaz. Önizleme atlanan her şeyi listeler.

### HAR dosyası

DevTools → Network → *Save all as HAR* ile bir HAR kaydedin ve yolunu girin.
**Only XHR/fetch requests (skip pages, scripts, styles, images)** varsayılan
olarak açıktır. Yalnızca istekler okunur, yanıtlar asla. Dosya en fazla 50 MiB
olabilir; bir planda en fazla 500 istek bulunur.

cURL ve HAR aktarımlarında Authorization, Cookie ve benzeri header'lar secret
olarak işaretlenir: bu oturumda çalışırlar ama kaydedilmezler.

### OpenAPI (JSON/YAML)

![OpenAPI dosyasıyla Import requests: sunucu seçimi, operasyon filtreleri ve seçim, uyarılar](screenshots/guide/05-import-openapi.png)

1. **OpenAPI (JSON/YAML)** seçin ve bir OpenAPI 3.x ya da Swagger 2.0
   dosyasının yolunu girin (`.json`, `.yaml`, `.yml`).
2. İsterseniz bir **Base URL** girin; örneğin yerel ya da staging adresi.
   Belgedeki sunucunun yerine geçer.
3. **Preview**'a tıklayın. Şunları görürsünüz:
   - belgede birden fazla sunucu varsa bir **Server:** açılır listesi ve `enum`
     tanımlı her sunucu değişkeni için bir liste (birden fazla scheme içeren
     Swagger 2.0 dosyalarında scheme listesi). **Requests go to** ortaya çıkan
     temel URL'yi ve nereden geldiğini gösterir;
   - belgedeki sırasıyla operasyonlar (GET/POST/PUT/PATCH/DELETE): metot, path,
     ad ve etiketlerle. Hepsi seçili başlar;
   - bir metin filtresi (**Filter by path, name or operationId**), **All
     methods** ve **All tags** filtreleri ve o an görünen operasyonlara uygulanan
     **Select all (N)** / **None** düğmeleri;
   - "N of M operations selected" bilgisi; seçim planı 500 isteğin üzerine
     çıkaracaksa bir hata mesajı.
4. **Import**'a tıklayın.

Belge şöyle çevrilir:

- Path parametreleri, örnek değerleriyle plana eklenen `{{değişken}}`'lere
  dönüşür. Zorunlu sorgu ve header parametreleri `example`, `examples`,
  `default` ya da `enum` değerlerini kullanır.
- JSON gövdeler belgedeki örnekleri kullanır. Örnek yoksa türe göre yer
  tutucular doldurulur (`""`, `0` veya `minimum`, `false`, tek öğeli diziler;
  date, date-time, uuid ve email biçimleri için sabit değerler). Önizleme bu
  gövdeleri sayar; yer tutucuları gerçek test verisiyle değiştirin.
- Bearer ve OAuth güvenliği `{{access_token}}` olur; header'daki ya da sorgu
  parametresindeki API anahtarları `{{api_key}}` olur (diğer şemalar için
  `{{api_key_2}}`, …). Bunlar sizin dolduracağınız secret plan değişkenleri
  olarak eklenir. Basic Auth için kimlik bilgilerini girmeniz gerekir. Cookie'deki
  API anahtarları, diğer şemalar ve JSON olmayan gövdeler önizlemede listelenir.
- Belgenin klasöründeki (ve alt klasörlerindeki) diğer JSON veya YAML
  dosyalarına verilen `$ref`'ler izlenir. Klasör dışındaki dosyalar, mutlak
  yollar ve `http(s)` referansları okunmaz; çözülemeyen referanslar adlarıyla
  listelenir. Her biri en fazla 10 MiB olmak üzere en fazla 50 dosya okunur.
- YAML, YAML 1.2 core şemasına göre okunur: `yes`/`no`/`on`/`off` metin olarak
  kalır; tırnaksız `openapi: 3.1` veya `swagger: 2.0` kabul edilir. Hatalar
  satır ve sütun bilgisiyle gösterilir, dosya içeriği asla gösterilmez.

## 10. Kişisel dashboardlar

Studio, Studio 2 ve Monitor'ün her birinin kendi widget sayfaları vardır. Bir
çalışma alanında yaptığınız değişiklik diğerlerini etkilemez. Düzenler planlardan
ayrıdır: bir planı kaydetmek düzenleri içermez.

| Çalışma alanı | Sabit kısımlar | Widget alanları (dock) |
| --- | --- | --- |
| Studio | Plan listesi, istek düzenleyici, Load Configuration | Bottom |
| Studio 2 | Plan ağacı ve düzenleyiciler (sayfa dashboard-only değilse) | Bottom, Right |
| Monitor | Çalıştırma listesi, başlık, uyarı, warmup ve SLO satırları | Main area, Bottom, Right |

### Widget'ları düzenleme

1. **Edit layout**'a tıklayın. Her widget'ta bir `::` sürükleme tutamacı, adı,
   bir **…** menüsü ve **×** görünür.
2. Bir `::` tutamacını başka bir widget'ın üzerine bırakınca widget onun önüne
   taşınır. Sürüklemeden yapmak için **…** → **Move earlier** / **Move later**
   kullanın.
3. **…** menüsünden genişlik için **Small**, **Medium** veya **Full row**; dock
   değiştirmek için **Move to Bottom** / **Move to Right** / **Move to Main area**
   seçin. (Right dock'taki widget'lar panel genişliğini paylaşır.)
4. **×** bir widget'ı gizler. Boşalan dock kaybolur ve yeri düzenleyicilere
   kalır.
5. Hemen kaydetmek için **Save layout**'a tıklayın. Düzenler uygulamayı
   kapattığınızda da kaydedilir.

Alt ya da sağ panelin kenarını sürükleyerek boyutunu değiştirebilirsiniz.

Fare bir widget'ın üzerindeyken (**Edit layout** dışında) üst kenarının
sağında **Expand** belirir; böylece widget'ın kendi kontrollerini kapatmaz.
Widget'ı pencerenin ortasında büyük olarak, canlı şekilde
açar; **Close**, Esc ya da dışarıya tıklama geri döndürür. Düzen değişmez.

### Widget ekleme ve sayfaları yönetme

**+ Widget** geçerli sayfaya bir widget ekler (Studio 2 ve Monitor'de dock başına
**Add to …** alt menüleriyle). **Layouts…**, **My Studio layouts & widgets**
penceresini (ya da Studio 2 / Monitor karşılığını) açar:

![Studio için düzen yöneticisi: widget listesi ve widget kataloğu](screenshots/guide/06-layout-manager.png)

- Sayfa seçici, **Save layout** ve **Restore default widgets** (çalışma alanının
  varsayılan widget'larını bu sayfaya geri koyar).
- **Page name** alanıyla **Create blank page**, **Duplicate current page** ve
  **Rename current page**; **Delete page** (en az bir sayfa kalmalıdır).
- Her widget için bir satır: göstermek ya da gizlemek için bir onay kutusu
  (gizlenen widget'lar buradan geri gelir), dock'u, genişliği, **Up**, **Down**
  ve **Remove** (widget'ın yapılandırmasını bu sayfadan siler).
- Her widget türü için bir **+** düğmesi bulunan **Widget catalog**.
- Yalnızca Studio 2: **Dashboard only (hide request editor)** ve **Dashboard row
  height**. Studio 2'de yeni boş sayfalar bu modda açılır; böyle bir sayfadan
  planı düzenlemek ve yetkilendirmek için **Configure test in Studio**'yu
  kullanın. Monitor'de **Main area row height** ayarı vardır.

Sayfalar arasında araç çubuğundaki **Layout: …** seçiciyle geçin; test sürerken
de olur.

### On widget

| Widget | Gösterdiği |
| --- | --- |
| **Throughput** | Zaman içinde saniye başına tamamlanan istek |
| **Requests per Second** | Toplam, başarılı ve başarısız hızlar (Monitor'ün grafiği) |
| **Latency** | Aralık bazında P50/P95/P99 |
| **Recent Events / Logs** | Filtrelenebilir log |
| **Active Users** | Zaman içinde sanal kullanıcılar |
| **HTTP Status Distribution** | Durum kodları, hata sayaçları ve hata nedenleri |
| **Endpoint Breakdown** | İstek bazında tablo |
| **Live Metrics** | Studio 2'nin metrik kartları |
| **KPI Cards** | Monitor'ün sekiz özet kartı |
| **Latency Distribution** | Tüm testin latency histogramı |

Studio ve Studio 2'de widget'lar mevcut çalıştırmayı, Monitor'de ise Monitor'de
açık olan çalıştırmayı (canlı ya da geçmişten) gösterir. Aynı widget'ı birden
fazla kez ekleyebilirsiniz. Log filtreleri ve **Follow** tüm log widget'ları
arasında ortaktır.

Çalışma alanı başına sınırlar: 20 sayfa, sayfa başına 16 widget, 1–60 karakterlik
benzersiz sayfa adları. Harici veri kaynağı, özel sorgu, eklenti ve serbest
konumlandırma yoktur.

## 11. Tema, tercihler ve kısayollar

- Sağ üstteki **☀ Light** / **☾ Dark** temayı değiştirir.
- Oturumlar arasında hatırlananlar: tema, son çalışma alanı, son planın yolu,
  dashboard sayfaları ve karşılaştırmadaki gürültü bandı. Bu tercihler plan,
  istek veya kimlik bilgisi içermez.
- Uygulama açılırken en son kaydettiğiniz veya açtığınız planı yeniden açar
  (**Open**'da olduğu gibi secret'ları ve oturumluk gövdeleri yeniden girmeniz
  gerekir). Dosya silinmiş ya da okunamıyorsa yeni bir planla başlar ve yolu
  unutur; böylece **Save** hiçbir dosyanın üzerine yazmadan nereye
  kaydedileceğini sorar.

| Kısayol | İşlev |
| --- | --- |
| Ctrl+S (macOS'ta ⌘S) | Planı kaydet (ilk seferde yol sorar) |
| Ctrl+O (⌘O) | Plan aç (kaydedilmemiş değişiklik varsa önce sorar) |
| Ctrl+Q (⌘Q) | Uygulamayı güvenle kapat |

Test sürerken Ctrl+S ve Ctrl+O çalışmaz. Kaydedilmemiş değişiklik ya da süren
bir test varken uygulamayı kapatırsanız **Close RustLoad?** penceresi **Keep
working**, **Save plan…** ve **Discard plan & close** veya **Stop & close**
seçeneklerini sunar. Süren test durdurulur ve kısmi sonuçları kaydedilir.

## 12. Sorun giderme ve SSS

### Sık görülen mesajlar

| Mesaj | Ne yapmalı |
| --- | --- |
| Confirm you are authorized to test every configured target. | Yetki kutusunu işaretleyin. Plandaki her değişiklikten sonra işaret kalkar |
| Re-enter secret value: *ad* | Secret bir parametre, header ya da değişken boş; genellikle kayıtlı bir plan açıldıktan sonra olur. Değeri girin ya da bir `{{değişken}}` referansı kullanın |
| Re-enter Basic Auth password / Re-enter Bearer token or variable reference | **Auth** sekmesini doldurun |
| Target scheme and host cannot contain variables | Şema ve host'u düz yazın; değişkenleri yalnızca path ve sorguda kullanın |
| Credentials in URLs are not allowed; use Auth | URL'deki `kullanici:parola@` kısmını kaldırıp **Auth**'u kullanın |
| Move sensitive URL parameters to Params and mark them secret | Örneğin `?token=` parametresini URL'den **Params**'a taşıyın |
| Only HTTP and HTTPS targets are supported / Invalid absolute URL | Tam bir `http://` veya `https://` URL'si kullanın |
| CSV needs one row per user, or enable Reuse rows | Satır ekleyin ya da **Reuse rows when there are more users than data rows** kutusunu işaretleyin |
| CSV file cannot be opened | Yolu kontrol edin; yol, kaydedilen planın klasörüne göredir |
| Ramp-up must be between 1s and the total duration | **Ramp-up**'ı kısaltın ya da **Total duration**'ı uzatın |
| Warmup duration must be at least 1s and shorter than the total duration | **Warmup**'ı düzeltin |
| Rate stages need a peak rate above 0 iterations/s | En az bir hız aşamasının hedefini 0'dan büyük yapın |
| JSON assertion expected value must be valid JSON | Metni tırnakla yazın: `ok` değil `"ok"` |
| Extractor requires a variable name and RFC 6901 JSON pointer | **Variable**'ı doldurun; pointer `/` ile başlar |
| An SLO threshold refers to a request that no longer exists | O eşiği silin ya da kapsamını değiştirin |
| Unsupported schema version; no automatic migration available | Plan dosyası uyumsuz bir sürümden geliyor |

### Çok sayıda düşürülen iterasyon

İterasyonların başlama zamanı geldiğinde tüm **Max VUs** meşguldü: hedef,
hız × VU sayısının kaldırabileceğinden yavaş. **Max VUs**'u artırın, hızı
düşürün ya da bunu bir bulgu olarak değerlendirin (hedef o hızı
kaldıramıyor). Düşürülen iterasyonlar hata sayılmaz, ama uygulanan yük
planlanandan düşük kalmıştır ve karşılaştırmalar bu konuda uyarır.

### Yüksek hata oranı

Önce durum panelindeki **Failure reasons** listesine bakın:

- **HTTP 4xx/5xx, expected 2xx**: hedef isteği reddetti. Auth'u, eksik
  değişkenleri ve test verisini kontrol edin. Bilerek beklediğiniz bir kod içinse
  **Exact expected status** ayarlayın.
- **Timeout**: hedef **Request timeout**'tan yavaş.
- **Connection refused** / **Connection reset or closed** / **Connection failed
  (DNS, TLS or TCP)**: sunucu kapalı, port yanlış ya da sunucu yük altında
  bağlantıları düşürüyor. Yeniden başlatmadan sonra kayıtlı bir Local Demo planı
  her zaman burada takılır, çünkü portu değişmiştir.
- **ResponseTooLarge** (**Body limit** sayacı): **Body cap**'i artırın.
- **Invalid template, URL or header (check variables)** (**Local
  preparation**): bir değişken eksik; örneğin daha önceki bir extraction başarısız
  olmuş.
- **Extraction found nothing at …** / **JSON at … did not match**: yanıt
  senaryonun beklediği gibi değil.

Durum satırına da bakın: "Local capacity warning: results may be
generator-limited" uyarısı, RustLoad'un kendisinin CPU ya da bellek sıkıntısı
çektiğini gösterir; bu durumda sayılar hedef kadar sizin bilgisayarınızı da
anlatır.

### Geçmiş kaydı sorunları

SQLite'a yazma başarısız olursa Monitor'de "History writer unavailable; export
results before closing" ya da "History persistence failed; export current results
before closing" yazar. Test devam eder ve sonuçlar bellekte kalır: kapatmadan
önce **Export Report** ile dışa aktarın. Boş disk alanını ve veri klasörünün
izinlerini kontrol edin.

### Verilerim nerede saklanıyor?

- **Çalıştırma geçmişi**: RustLoad veri klasöründeki `history.sqlite3`
  (Test Runs'taki **Open data folder** bu klasörü açar). macOS'te
  `~/Library/Application Support/dev.RustLoad.RustLoad/history.sqlite3`;
  Linux'ta `~/.local/share/rustload/` (ya da `$XDG_DATA_HOME/rustload/`);
  Windows kullanıcıya ait olağan uygulama verisi klasörünü kullanır.
- **Tercihler ve düzenler**: eframe'in `app.ron` dosyası; Linux'ta aynı klasörde.
- **Planlar ve raporlar**: nereye kaydettiyseniz orada.

Geçmişi temizlemek için RustLoad'u kapatın ve `history.sqlite3` dosyasını silin
ya da başka bir yere taşıyın.

#### Geçmişi sorgulama

`history.sqlite3` sıradan bir SQLite dosyasıdır. `runs` ve `points` tabloları
her çalıştırmanın özetini ve saniyelik noktalarını JSON olarak tutar (uygulama
bunları okur). Yanlarında kendi sorgularınız için, çalıştırma bittiğinde
doldurulan düz tablolar vardır:

| Tablo | Satır başına | Başlıca sütunlar |
| --- | --- | --- |
| `run_metrics` | çalıştırma | `name`, `started_at` (UTC), `state`, `elapsed`, `measured_secs`, `started`, `completed`, `successful`, `failed`, `cancelled`, `error_rate` (%), `throughput` (ölçüm penceresinde istek/sn), `p50`, `p90`, `p95`, `p99`, `mean` (ms), `bytes`, `warmup_started`, `warmup_completed`, `dropped_iterations`, `slo_verdict` |
| `run_endpoints` | çalıştırmadaki istek | `request_id`, `name`, `method`, `completed`, `failed`, `error_rate`, `p50`–`p99`, `mean`, `bytes` |
| `run_slo` | çalıştırmadaki SLO | `position`, `metric`, `scope_request_id`, `target`, `unit`, `actual`, `status` |
| `run_errors` | isteğin hata nedeni | `request_id`, `reason`, `count`, `first_at`, `last_at` |

Hepsi `run_id` sütununu (çalıştırmanın `runs` içindeki kimliği) kullanır.
Warmup istekleri sayaçlara ve gecikmelere girmez. Ölçüm yoksa gecikme ve hata
oranı sütunları boştur. Tablolarda yalnızca maskelenmiş özet bulunur: adlar,
metotlar, kimlikler, sayılar ve temizlenmiş hata nedenleri; secret, header, URL
ya da gövde yoktur. Eski sürümlerin geçmiş dosyaları RustLoad onları açtığında
yükseltilir ve önceki çalıştırmalar doldurulur. Yükseltme başarısız olursa Test
Runs bir uyarı gösterir; çalıştırmalar yine açılır ve kaydedilir.

Örnek (tamamlanan son 10 çalıştırmanın P95'i); `sqlite3` aracında ya da herhangi
bir SQLite tarayıcısında. Dışa aktarılmış bir kopyayı sorgulayın ya da önce
RustLoad'u kapatın:

```sql
SELECT started_at, name, p95, error_rate, throughput
FROM run_metrics
WHERE state = 'Completed'
ORDER BY started_at DESC
LIMIT 10;
```

Bir isteğin zaman içindeki değeri:

```sql
SELECT m.started_at, e.name, e.p95
FROM run_endpoints e JOIN run_metrics m USING (run_id)
WHERE e.name = 'Checkout'
ORDER BY m.started_at DESC;
```

### Bilmeniz gereken sınırlar

- Otomatik yeniden deneme, yönlendirme takibi, cookie jar ya da çerez
  oturumu yoktur. TLS sertifikaları her zaman doğrulanır.
- Uzak hedefler sistemin HTTP proxy'sini kullanır; doğrudan yazılmış loopback
  adresleri kullanmaz.
- Senaryolar yalnızca sıralıdır: akış, adımlar arası bekleme, döngü, koşul ya da
  şablon yoktur. Dağıtık worker veya tarayıcı emülasyonu da yoktur.
- Grafikler 900 nokta, loglar 2 KiB'a kadar 5.000 kayıt tutar.
- Girdi olarak en fazla 10.000 kullanıcı ve saniyede 100.000 istek başlangıcı
  kabul edilir; bunlar doğrulama sınırlarıdır, ölçülmüş kapasite değildir.
  Kapasiteyi kendi donanımınızda ölçün.
- Durum satırındaki CPU ve RAM, RustLoad'un kendi kullanımıdır. Local Demo
  sunucusunu aynı işlemde çalıştırdığı için onun kullanımı da buna dahildir.

## 13. Metrik sözlüğü

| Terim | Anlamı |
| --- | --- |
| **Started** | Gönderilen istekler: tamamlanan + iptal edilen + devam eden |
| **Completed** | Bir sonuçla biten istekler: başarılı + başarısız |
| **Successful** | Beklenen durum kodu (varsayılan 200–299), gövde tamamen okunmuş, tüm assertion ve extraction'lar geçmiş |
| **Failed** | Tamamlanmış ama başarısız: beklenmeyen durum kodu, assertion ya da extraction hatası, timeout, ağ hatası, gövde sınırı, hazırlık hatası |
| **Cancelled** | Stop ya da test sonu nedeniyle bitmeden kesilen; hata oranına girmez |
| **In-flight** | Başlamış, henüz bitmemiş |
| **Error rate** | Başarısız ÷ tamamlanan, % |
| **Requests / sec (kart)** | Son 5 saniyede saniye başına tamamlanma |
| **Throughput (grafik)** | Test boyunca aralık başına saniyedeki tamamlanma |
| **Throughput (SLO, karşılaştırma)** | Tamamlanan istek ÷ ölçüm süresi (warmup sonrası) |
| **Latency** | HTTP isteğinin başlamasından yanıt gövdesinin tamamen okunmasına kadar geçen süre. Pacing ve think time dahil değildir. Yalnızca tamamen okunan yanıtlar sayılır; timeout'lar ve ağ hataları girmez |
| **P50 / P90 / P95 / P99** | Yanıtların %50/90/95/99'u en az bu kadar hızlı. Kartlar ve SLO'lar tüm-test histogramını, grafikler aralık başına ayrı histogramları kullanır |
| **Mean / Average Response Time** | Ölçülen yanıtların aritmetik ortalama latency'si |
| **Data / sec** | Son 5 saniyede saniye başına yanıt gövdesi byte'ı; header'lar hariç |
| **Sanal kullanıcı (VU)** | Senaryoyu kendi değişkenleriyle sırayla çalıştıran simüle istemci |
| **İterasyon** | Bir VU'nun tüm istekleri bir kez çalıştırması |
| **Dropped iterations** | Yalnızca Arrival Rate: tüm Max VUs meşgul olduğu için başlatılmayan iterasyonlar |
| **Warmup** | İstekleri sonuçların dışında tutulan ve ayrı sayılan başlangıç dönemi |
| **Think time** | Kapalı modellerde her iterasyondan sonraki bekleme |
| **Global RPS cap** | Kapalı modeller: tüm kullanıcılar için saniyedeki istek başlangıcı sınırı |
| **SLO sonucu** | Planın eşiklerine göre Passed, Failed ya da Inconclusive |

Tasarım ve ölçüm ayrıntıları için [README](../README_tr.md),
[mimari](architecture.md), [Studio 2](studio2.md) ve [widget](widgets.md)
belgelerine bakın.
