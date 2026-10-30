# RustLoad

[English](README.md) · [Türkçe](README_tr.md)

RustLoad, HTTP API'lerini yük altında test etmek için geliştirilmiş bir masaüstü
uygulaması. Ardışık isteklerden bir senaryo oluşturabilir, eşzamanlı sanal
kullanıcılarla çalıştırabilir ve test sürerken yanıt sürelerini, istek hızını
ve hataları izleyebilirsiniz.

Uygulama Rust ve egui/eframe ile yazıldı. Yük motoru kendi bilgisayarınızda
çalışır; geçmişi SQLite'ta saklar, raporları çevrimdışı açabilirsiniz. Hesap,
ayrı bir backend servisi veya Docker gerekmez.

[Başlangıç](#başlangıç) · [Çalışma alanları](#çalışma-alanları) ·
[Kişisel dashboardlar](#kişisel-dashboardlar) · [Ölçümler](#sonuçlar-nasıl-ölçülür) ·
[Geliştirme](#geliştirme)

<picture>
  <source media="(prefers-color-scheme: light)" srcset="docs/screenshots/monitor-light.png">
  <img src="docs/screenshots/monitor-dark.png" alt="RustLoad Monitor ekranında tamamlanmış bir yerel testin gecikme yüzdelikleri, istek hızı, HTTP durum kodları, uç nokta sonuçları ve logları">
</picture>

*Yerel demo tamamlandıktan sonra Monitor ekranı. Bu sayfadaki ekran görüntüleri,
uygulamayla gelen HTTP test sunucusu kullanılarak Linux üzerinde alındı.*

## Neler yapabilirsiniz?

| İşlem | RustLoad'da karşılığı |
| --- | --- |
| HTTP senaryosu oluşturma | Ardışık GET, POST, PUT, PATCH ve DELETE istekleri; header, sorgu parametresi, ham veya JSON gövde, Basic ve Bearer kimlik doğrulaması |
| İstekler arasında veri aktarma | Değişkenler, kullanıcıya atanmış CSV satırları ve erişim token'ı gibi değerler için JSON Pointer ile veri çıkarma |
| Yanıtları doğrulama | Beklenen durum kodu, yanıt süresi sınırı ve JSON Pointer eşitlik kontrolleri |
| Yükü yapılandırma | Eşzamanlı kullanıcı sayısı, sabit veya doğrusal artış profili, toplam süre, senaryo tekrarları arasındaki bekleme, zaman aşımı ve ortak istek başlatma sınırı |
| Testi izleme | İstek hızı, gecikme yüzdelikleri, aktif kullanıcılar, HTTP durum kodları, uç nokta özetleri ve filtrelenebilir loglar |
| Çalışma alanını düzenleme | İki Studio düzeni, adlandırılabilir widget sayfaları, sürükleyerek sıralama, ayarlanabilir genişlikler, koyu ve açık tema |
| Sonuçları saklama | Yerel SQLite geçmişi ve HTML, JSON veya CSV olarak dışa aktarma |

RustLoad şu anda MVP aşamasında. Linux üzerinde çalışması kontrol edildi.
Hedef platform olan macOS'ta yerel kabul testleri henüz yapılmadı. Bir sürüm
hazırlamadan önce [platform durumuna](#platform-durumu) bakın.

## Başlangıç

Rust'ı [rustup](https://rustup.rs) ile kurun, ardından depoyu klonlayın:

```sh
git clone https://github.com/hakanngul/RustLoadTesting.git
cd RustLoadTesting
```

Masaüstü uygulamasını başlatın:

```sh
cargo run --locked
```

`rust-toolchain.toml`, Rust 1.99.1 sürümünü seçer. İlk derlemede bu araç zinciri
ve sürümleri `Cargo.lock` içinde sabitlenmiş bağımlılıklar indirilir.

Uygulamayı açmak için bir grafik oturumu ve OpenGL gerekir. Linux'ta
X11/Wayland, xkbcommon ve GL çalışma zamanı kütüphaneleri de kurulu olmalı.
macOS'ta bağlayıcı için Xcode Command Line Tools'u (`xcode-select --install`)
kurun.

### Yerel demoyu deneyin

Demo, harici bir servise trafik göndermeden çalışan bir senaryoyu denemenizi
sağlar. Arayüzdeki düğme ve seçenek adları İngilizcedir.

1. RustLoad'u açın. Planda kaydedilmemiş değişiklik yokken **Local Demo**
   düğmesine tıklayın. Uygulama geçici bir `127.0.0.1` portunda test sunucusu açar.
2. Login, List products ve Checkout isteklerini inceleyin. Login yanıtından
   `/access_token` çıkarılır; ürün isteği `{{access_token}}` değerini, checkout
   ise `product_id` değişkenini kullanır.
3. Studio'nun yük yapılandırma panelindeki **I am authorized to test all targets
   in this plan.** kutusunu işaretleyin ve **Run Test** düğmesine tıklayın.
4. Sonuçları Monitor'dan takip edin. Demo 10 sanal kullanıcıyla 30 saniye çalışır;
   saniyede en fazla 20 HTTP isteği başlatır.
5. Testin bitmesini bekleyin veya **Stop** ile durdurup o ana kadarki sonuçları
   saklayın. **Export Report** ile HTML, JSON veya CSV kaydedebilir, geçmiş
   panelinden önceki bir çalıştırmanın özetini yeniden açabilirsiniz.

Kaydedilen demo planı geçici sunucu portunu da içerir. Uygulamayı yeniden
başlattığınızda bu port değişir. Sabit portlu bir sunucu için başka bir
terminalde şu komutu çalıştırın:

```sh
cargo run --locked --example fixture
```

Test sunucusu Ctrl-C ile kapatılana kadar `127.0.0.1:8080` adresinde dinler.
Plandaki istek URL'lerini bu adrese göre değiştirin. Kendi testlerinizde yalnızca
test etme izniniz olan hedefleri kullanın.

## Çalışma alanları

### Studio

Studio, ana senaryo editörüdür. Soldaki listeden bir istek seçin, ortadaki
alandan parametrelerini düzenleyin ve sağdan kullanıcı sayısı ile süreleri
ayarlayın. Bir isteğe sağ tıklayarak sırasını değiştirebilir, çoğaltabilir veya
silebilirsiniz.

![Koyu temada Studio: istek gövdesi editörü ve yük yapılandırma paneli](docs/screenshots/studio-dark.png)

İki Studio düzeni aynı test planını ve seçili isteği paylaşır. Test başladığında
yapılandırma, çalışma bitene kadar kilitlenir. Studio'dan başlatılan test Monitor'ı
açar; Studio 2'den başlatıldığında çalışma alanı açık kalır.

### Studio 2

Studio 2, istek ağacını, HTTP editörünü, gövdeyi ve yük yapılandırmasını birlikte
gösterir. Canlı metrikler sağda, grafikler ve loglar alttadır. Düzenleme için daha
fazla alan gerektiğinde bu widgetları gizleyebilir veya taşıyabilirsiniz.

![Yerel test sırasında Studio 2: istek ağacı, Body ve Load Configuration panelleri, canlı metrikler ve alttaki grafikler](docs/screenshots/studio2-dark.png)

Dar pencerelerde paneller daraltılabilir hale gelir veya sekmelere dönüşür.
Ayrıntılar için [Studio 2 rehberine](docs/studio2.md) ve
[küçük pencere görüntüsüne](docs/screenshots/studio2-compact.png) bakabilirsiniz.

### Monitor

Monitor, mevcut testin istek hızını, P50/P95/P99 gecikmelerini, hata oranını ve
istek sayısını gösterir. Grafiklerden istek hızını, ölçüm aralıklarına ait
gecikmeleri ve aktif kullanıcı sayısını izleyebilirsiniz. HTTP durum kodları,
uç nokta sonuçları ve filtrelenebilir loglar hataları incelemenize yardımcı olur.
Kaydedilmiş bir özeti açmak için soldaki geçmişten bir çalıştırma seçin.

Uygulamanın tamamında koyu ve açık tema kullanılabilir. Açık temadaki
[Studio](docs/screenshots/studio-light.png),
[Studio 2](docs/screenshots/studio2-light.png) ve
[Monitor](docs/screenshots/monitor-light.png) görüntülerini de inceleyebilirsiniz.

## Kişisel dashboardlar

Studio 2'de farklı işler için ayrı düzenler kaydedebilirsiniz. İstekleri
düzenlerken yalnızca istek hızı grafiğini açık bırakabilir veya sonuçları izlemek
için ayrı bir sayfa oluşturabilirsiniz.

![İstek hızı, HTTP durum dağılımı, aktif kullanıcılar ve loglardan oluşan Performance dashboard'u](docs/screenshots/widgets-dashboard.png)

Mevcut sayfayı düzenlemek için:

1. Sürükleme tutamaçlarını ve widget kontrollerini açmak için **Edit layout**
   düğmesine tıklayın.
2. **+ Widget** ile grafik, tablo, log paneli veya canlı metrik grubu ekleyin.
3. Widgetları sürükleyerek sıralayın. Menülerinden satır genişliğini ayarlayabilir,
   editörlü düzende alt ve sağ paneller arasında taşıyabilirsiniz.
4. Bir widgetı gizlemek için **×** düğmesine tıklayın. **Layouts…** üzerinden
   yeniden gösterebilirsiniz. Boş kalan paneller kapanır, alan editörlere kalır.
5. Düzeni bu cihazda saklamak için **Save layout** düğmesine tıklayın.

Ayrı bir sayfa oluşturmak için **Layouts…** menüsünden **Create blank page**
veya **Duplicate current page** seçeneğini kullanın. Yeni boş sayfalar, istek
editörünü gizleyen dashboard modunda açılır. Sayfaları yeniden adlandırabilir ve
test sürerken aralarında geçiş yapabilirsiniz.

<details>
<summary>Widget düzenleme kontrollerini göster</summary>

![Dashboard düzenleme modunda sürükleme tutamaçları, widget menüleri ve gizleme düğmeleri](docs/screenshots/widgets-edit.png)

</details>

Katalogda yedi widget var: istek hızı, gecikme, loglar, aktif kullanıcılar, HTTP
durum dağılımı, uç nokta özeti ve canlı metrikler. Hepsi mevcut çalıştırmanın
verilerini okur. Aynı widgettan birden fazla ekleyebilirsiniz. Bu sürümde log
filtreleri ve otomatik takip ayarı, log widgetları arasında ortaktır.

En fazla 20 düzen, her düzende 16 widget bulunabilir. Sayfa adları benzersiz ve
1 ile 60 karakter arasında olmalı. Düzenler test planlarından ayrı kaydedilir;
planı JSON olarak kaydetmek dashboard'u dışa aktarmaz.
[Widget rehberinde](docs/widgets.md) boyutlandırma, varsayılanları geri yükleme
ve kayıt davranışı anlatılıyor. Harici veri kaynakları, özel sorgular, eklentiler
ve piksel bazında serbest yerleşim henüz uygulanmadı.

## Senaryolarla çalışma

URL yolunda ve sorgu değerlerinde, header alanlarında, istek gövdesinde veya
kimlik doğrulamada `{{variable}}` kullanabilirsiniz. URL'nin şeması ve sunucu
adı sabit olmalı. Eksik bir değişken, ilgili adımın istek hazırlama hatasıyla
başarısız olmasına yol açar. Herhangi bir adım başarısız olursa o senaryo
tekrarındaki kalan istekler atlanır.

Yanıttan değer çıkarmak için `/access_token` veya `/items/0/id` gibi RFC 6901
JSON Pointer ifadeleri kullanılır. Burada JSON Pointer geçerlidir, JSONPath
desteklenmez. Doğrulamalarla beklenen durum kodunu, yanıt süresi üst sınırını
ve bir JSON Pointer değerinin eşitliğini kontrol edebilirsiniz.

CSV sütun adları, her sanal kullanıcıya özel değişkenlere dönüşür. Kullanıcı
test boyunca aynı satırı kullanır. Satırların yeniden kullanılması ayrıca
etkinleştirilir; aynı sütun adı birden fazla kez bulunamaz. Göreli CSV yolları,
kaydedilmiş planın bulunduğu dizine göre çözülür. Plan henüz kaydedilmediyse
uygulama işleminin çalışma dizini esas alınır.

Planlar şema sürümü 1'i kullanır. Bilinmeyen sürümler reddedilir. Açma ve
kaydetme işlemleri dosya yolu girilen bir pencereden yapılır. Cmd/Ctrl+S
kaydeder, Cmd/Ctrl+O açar, Cmd/Ctrl+Q güvenli kapatmayı başlatır. Kapanışta
kaydedilmemiş değişiklikler için uyarı verilir ve aktif çalışma durdurulur.

### Kimlik bilgileri ve kaydedilen veriler

Doğrudan girilen gizli kimlik bilgileri ve gizli işaretlenmiş alanlar planlara,
geçmişe veya raporlara yazılmaz. Yalnızca `{{access_token}}` gibi bir değişken
referansı içeren kimlik doğrulama alanlarında bu referans korunur. Planı yeniden
açtığınızda kaydedilmeyen gizli değerleri tekrar girmeniz gerekir.

Gövdeyi kaydetme seçeneğini açmadığınız sürece istek gövdeleri yalnızca oturumda
kalır. Kimlik bilgilerini kaydedilen gövdeye yazmak yerine gizli değişkenler
kullanın. Geçmişe alınan yapılandırma kopyaları, istek gövdelerini ve URL sorgu
dizelerini içermez. Yanıt gövdeleri loglara yazılmaz.

## Sonuçlar nasıl ölçülür?

RustLoad kapalı döngü modeliyle çalışır: her sanal kullanıcı senaryoyu sırayla
tamamlar, ardından bir sonraki tekrar için ayarlanan süreyi bekler. Toplam test
süresine kullanıcıların kademeli artış süresi de dahildir. Kullanıcılardan
bağımsız, açık döngülü bir istek varış hızı üretilmez.

| Ölçüm | Anlamı |
| --- | --- |
| Başarılı istek | Varsayılan olarak HTTP 200 ile 299 arası veya ayrıca belirlenen beklenen durum kodu; tüm doğrulama ve veri çıkarma işlemlerinin de başarılı olması gerekir |
| Tamamlanan istekler | Başarılı ve başarısız isteklerin toplamı; iptal edilen ve devam eden istekler ayrı sayılır |
| Hata oranı | Başarısız istek sayısının tamamlanan isteklere oranı; iptaller hesaba katılmaz |
| HTTP hata sayıları | Başarı koşulundan bağımsızdır: beklenen bir 401 başarılı sayılabilir ve HTTP durum kodu sayımında da görünür |
| Yanıt gecikmesi | HTTP isteğinin yürütülmeye başlamasından yanıt gövdesinin tamamen okunmasına kadar geçen süre; hız sınırlaması ve senaryo arası bekleme hariçtir, ağ hataları ve zaman aşımları yanıt histogramlarına girmez |
| Yüzdelik kartları | Tüm çalıştırmanın HDR Histogram sonuçları |
| Yüzdelik grafikleri | Her ölçüm aralığına ait bağımsız histogramlar; aralıkların yüzdelikleri ortalanmaz |
| RPS kartı | Son beş saniyedeki tamamlanmalardan hesaplanan saniyelik istek sayısı; pencere sınırındaki kovalar kapsanan süreye göre ağırlıklandırılır |
| Ortak RPS sınırı | Tüm kullanıcıların HTTP istek başlangıçlarını zamanlar; zamanlayıcı gecikmesinin en fazla 10 ms'lik kısmı telafi edilir, boşta geçen süre toplu istek başlatma hakkı biriktirmez |

P50/P90/P95/P99 için HDR histogramları kullanılır. Değerler mikrosaniye olarak
kaydedilir, milisaniye olarak gösterilir. Örnek bulunmayan ölçümlerde yer tutucu
görünür. **Stop**, yeni istekleri engeller ve hız sınırlaması ile senaryo arası
beklemeleri keser. Devam eden isteklere tamamlanmaları için en fazla beş saniye
tanınır, ardından kalanlar iptal edilir. Son metrikler, sanal kullanıcı görevleri
sonlandıktan sonra kesinleşir.

Kullanıcının durdurduğu test **Cancelled**, süresi dolarak biten test **Completed**,
motorun iç hatasıyla sonlanan test **Failed** olarak işaretlenir. Süresi dolan
testlerde de devam eden istekler için aynı ek bekleme süresi uygulanır.

Sürüm 0.1'de otomatik yeniden deneme, yönlendirme takibi ve çerez yönetimi yoktur.
TLS doğrulaması açıktır. Uzak istekler sistemden devralınan HTTP proxy ayarlarını
kullanır; doğrudan yazılmış loopback adresleri için yerel istemci kullanılır.

Kapalı döngü testler, hedef aşırı yük altındayken gecikmeyi olduğundan düşük
gösterebilir (coordinated omission). Erişilen istek hızı, yük üreten bilgisayarın
CPU'suna, zamanlamasına, bağlantılarına ve hedefin davranışına da bağlıdır.
CPU ve RAM göstergeleri hedef sunucuyu değil, RustLoad işlemini ölçer. Local Demo
ve dayanıklılık testi, test sunucusunu aynı işlemde çalıştırdığı için bu ölçümler
sunucunun kaynak kullanımını da içerir.

## Saklama ve sınırlar

Çalıştırma geçmişi, işletim sisteminin RustLoad için ayırdığı yerel veri dizininde
`history.sqlite3` dosyasında tutulur. SQLite'a test sırasında toplu ölçümler,
test sonunda da sonuç özeti yazılır. Yazma başarısız olursa uygulama bunu bildirir;
mevcut sonuçları yine dışa aktarabilirsiniz. Arayüz tercihleri eframe'in platforma
özgü depolamasında saklanır.

| Öğe | Sınır |
| --- | --- |
| Canlı grafik geçmişi | 900 veri noktası |
| Loglar | 5.000 kayıt, mesaj başına en fazla 2 KiB |
| Yanıt gövdesi | Varsayılan 10 MiB, en fazla 100 MiB olarak ayarlanabilir; sınırı aşan yanıt hata sayılır |
| Plan veya CSV girdi dosyası | 16 MiB |
| Plandaki istekler | 500 tanım |
| Yapılandırma sınırları | En fazla 10.000 sanal kullanıcı ve saniyede 100.000 istek başlangıcı |

Yapılandırma sınırları, girilen değerleri doğrulamak içindir; ölçülmüş kapasite
sonuçları değildir. Kapasiteyi kendi donanımınızda ölçün. HTML raporları sonuçları
ve zaman serilerini içerir; metinlerdeki HTML özel karakterleri güvenli biçimde
kodlanır. Bu raporlarda etkileşimli bir dashboard bulunmaz.

## Geliştirme

Kontrolleri depo kök dizininde çalıştırın:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked
```

Testler yerel HTTP sunucularını kullanır. Motoru 15 dakikalık bir dayanıklılık
testinden geçirmek için:

```sh
cargo run --locked --example soak -- 900 /tmp/rustload-soak
```

Dayanıklılık testi RSS, CPU ve sayaçları kaydeder; istek sayılarının tutarlılığını
denetler ve çevrimdışı bir JSON raporu yazar. Motoru ve yerel test sunucusunu
çalıştırır; arayüzün çizim katmanını kapsamaz. Kaydedilmiş komutlar ve sonuçlar
[doğrulama belgesinde](docs/validation.md) bulunuyor.

Motor, ayrı bir Tokio runtime kullanır ve arayüze toplu ölçüm kopyaları gönderir.
Depolama yazmaları ayrı bir iş parçacığında yürütülür. Yürütme veya
ölçüm davranışını değiştirmeden önce [mimariyi](docs/architecture.md) ve
[uygulama kararlarını](docs/decisions.md) inceleyin.

### Platform durumu

| Platform | Durum |
| --- | --- |
| Linux | Geliştirme ortamında derleme, otomatik testler ve yerel arayüz kontrolleri kayıtlı |
| macOS | Hedef platform; uygulamanın açılması, manuel kabul testleri ve paketleme henüz doğrulanmadı |
| Windows | Çalıştırma ve paketleme doğrulanmadı |

Mac üzerinde `bash scripts/package-macos.sh`, imzasız bir `dist/RustLoad.app`
oluşturur. Betiğin Linux ortamında yalnızca sözdizimi kontrol edildi. İmzalama,
notarization ve dağıtım yapılandırılmadı. CI'da Linux ve macOS
işleri tanımlı; gerçek workflow sonuçlarını ayrıca kontrol edin.

1.000 kullanıcıyla kapasite ve çizim katmanının uzun süreli bellek kullanımı
henüz ölçülmedi. Flow, Pause, şablonlar, tarayıcı emülasyonu, histogram
görselleştirmesi, çerez oturumları, dağıtık worker'lar ve performans testi
karşılaştırmaları mevcut uygulamanın kapsamı dışında.

## Belgeler

- [MVP kapsamı](docs/MVP.md)
- [Mimari](docs/architecture.md)
- [Uygulama kararları](docs/decisions.md)
- [Studio 2](docs/studio2.md)
- [Widgetlar ve kişisel düzenler](docs/widgets.md)
- [Test planı](docs/test-plan.md)
- [Doğrulama sonuçları ve kalan sürüm kontrolleri](docs/validation.md)
- [İlk ürün gereksinimleri](docs/original-prd.md)
