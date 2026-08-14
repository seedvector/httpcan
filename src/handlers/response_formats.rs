use super::*;
use flate2::{write::DeflateEncoder, write::GzEncoder, Compression};
use std::io::Write;
use zstd::stream::write::Encoder as ZstdEncoder;

pub async fn json_handler(_req: HttpRequest) -> Result<HttpResponse> {
    let sample_data = json!({
        "slideshow": {
            "author": "Yours Truly",
            "date": "date of publication",
            "slides": [
                {
                    "title": "Wake up to WonderWidgets!",
                    "type": "all"
                },
                {
                    "items": [
                        "Why <em>WonderWidgets</em> are great",
                        "Who <em>buys</em> WonderWidgets"
                    ],
                    "title": "Overview",
                    "type": "all"
                }
            ],
            "title": "Sample Slide Show"
        }
    });

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(sample_data))
}

pub async fn xml_handler(_req: HttpRequest) -> Result<HttpResponse> {
    let xml_content = r#"<?xml version='1.0' encoding='us-ascii'?>
<!--  A SAMPLE set of slides  -->
<slideshow 
    title="Sample Slide Show"
    date="Date of publication"
    author="Yours Truly"
    >
    <!-- TITLE SLIDE -->
    <slide type="all">
      <title>Wake up to WonderWidgets!</title>
    </slide>

    <!-- OVERVIEW -->
    <slide type="all">
        <title>Overview</title>
        <item>Why <em>WonderWidgets</em> are great</item>
        <item/>
        <item>Who <em>buys</em> WonderWidgets</item>
    </slide>
</slideshow>"#;

    Ok(HttpResponse::Ok()
        .content_type("application/xml")
        .body(xml_content))
}

pub async fn html_handler(_req: HttpRequest) -> Result<HttpResponse> {
    let html_content = r#"<!DOCTYPE html>
<html>
  <head>
  </head>
  <body>
      <h1>Herman Melville - Moby-Dick</h1>

      <div>
        <p>
          Availing himself of the mild, summer-cool weather that now reigned in these latitudes, and in preparation for the peculiarly active pursuits shortly to be anticipated, Perth, the begrimed, blistered old blacksmith, had not removed his portable forge to the hold again, after concluding his contributory work for Ahab's leg, but still retained it on deck, fast lashed to ringbolts by the foremast; being now almost incessantly invoked by the headsmen, and harpooneers, and bowsmen to do some little job for them; altering, or repairing, or new shaping their various weapons and boat furniture. Often he would be surrounded by an eager circle, all waiting to be served; holding boat-spades, pike-heads, harpoons, and lances, and jealously watching his every sooty movement, as he toiled. Nevertheless, this old man's was a patient hammer wielded by a patient arm. No murmur, no impatience, no petulance did come from him. Silent, slow, and solemn; bowing over still further his chronically broken back, he toiled away, as if toil were life itself, and the heavy beating of his hammer the heavy beating of his heart. And so it was.—Most miserable!
        </p>
        <p>
          A peculiar walk in this old man, a certain slight but painful appearing yawing in his gait, had at an early period of the voyage excited the curiosity of the mariners. And to the importunity of their persisted questionings he had finally given in; and so it came to pass that every one now knew the shameful story of his wretched fate.
        </p>
        <p>
          Belated, and not innocently, one bitter winter's midnight, on the road running between two country towns, the blacksmith half-stupidly felt the deadly numbness stealing over him, and sought refuge in a leaning, dilapidated barn. The issue was, the loss of the extremities of both feet. Out of this revelation, part by part, at last came out the four acts of the gladness, and the one long, and as yet uncatastrophied fifth act of the grief of his life's drama.
        </p>
        <p>
          He was an old man, who, at the age of nearly sixty, had postponedly encountered that thing in sorrow's technicals called ruin. He had been an artisan of famed excellence, and with plenty to do; owned a house and garden; embraced a youthful, daughter-like, loving wife, and three blithe, ruddy children; every Sunday went to a cheerful-looking church, planted in a grove. But one night, under cover of darkness, and further concealed in a most cunning disguisement, a desperate burglar slid into his happy home, and robbed them all of everything. And darker yet to tell, the blacksmith himself did ignorantly conduct this burglar into his family's heart. It was the Bottle Conjuror! Upon the opening of that fatal cork, forth flew the fiend, and shrivelled up his home. Now, for prudent, most wise, and economic reasons, the blacksmith's shop was in the basement of his dwelling, but with a separate entrance to it; so that always had the young and loving healthy wife listened with no unhappy nervousness, but with vigorous pleasure, to the stout ringing of her young-armed old husband's hammer; whose reverberations, muffled by passing into her ears the sweet home sounds, came to her not ungratefully in the roarings of the forge; only before that, and after that, the forge was but an uncomfortable part of this old man's story.
        </p>
      </div>
  </body>
</html>"#;

    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html_content))
}

/// `/robots.txt` — allows all crawlers, keeps `/deny` off-limits (httpbin
/// parity), advertises the sitemap with an absolute `Sitemap:` directive
/// (RFC 9309 / https://www.sitemaps.org/protocol.html), and declares AI usage
/// preferences via `Content-Signal` (https://contentsignals.org/): search
/// indexing, AI training, and model input all allowed.
pub async fn robots_txt_handler(
    req: HttpRequest,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse> {
    // User override: `static/robots.txt` replaces the generated policy
    // verbatim (e.g. a private instance can `Disallow: *`).
    if let Some(file) = static_override(&config, "robots.txt") {
        return override_response(file, "text/plain", &req);
    }
    let base = resolved_base(&req, &config);
    let robots_content = format!(
        "User-agent: *\n\
         Disallow: /deny\n\
         Content-Signal: ai-train=yes, search=yes, ai-input=yes\n\
         \n\
         Sitemap: {base}/sitemap.xml\n"
    );

    Ok(HttpResponse::Ok()
        .content_type("text/plain")
        .body(robots_content))
}

/// `/sitemap.xml` — a Sitemaps-protocol index of the site's canonical public
/// pages (https://www.sitemaps.org/protocol.html). Generated dynamically from
/// the request origin so every instance advertises its own absolute URLs and
/// stays in sync with whatever the homepage exposes. The homepage is the only
/// crawlable content page; the API endpoints (/get, /post, …) echo request
/// data and are intentionally not advertised to crawlers.
pub async fn sitemap_handler(
    req: HttpRequest,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse> {
    // User override: `static/sitemap.xml` replaces the generated index
    // verbatim. Note: absolute URLs in a static file do not follow the
    // request origin or `--canonical-scheme` — the operator owns them.
    if let Some(file) = static_override(&config, "sitemap.xml") {
        return override_response(file, "application/xml", &req);
    }
    let base = resolved_base(&req, &config);
    let body = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n\
         <url><loc>{base}/</loc></url>\n\
         </urlset>\n"
    );

    Ok(HttpResponse::Ok()
        .content_type("application/xml")
        .body(body))
}

pub async fn deny_handler(_req: HttpRequest) -> Result<HttpResponse> {
    Ok(HttpResponse::Ok()
        .content_type("text/plain")
        .body("YOU SHOULDN'T BE HERE"))
}

pub async fn utf8_handler(_req: HttpRequest) -> Result<HttpResponse> {
    let utf8_content = r#"<!DOCTYPE html>
<html>
  <head>
    <meta charset="UTF-8">
    <title>UTF-8 Test</title>
  </head>
  <body>
    <h1>Unicode Demo</h1>

    <p>Taken from <a
    href="http://www.cl.cam.ac.uk/~mgk25/ucs/examples/UTF-8-demo.txt">http://www.cl.cam.ac.uk/~mgk25/ucs/examples/UTF-8-demo.txt</a></p>

    <pre>

    UTF-8 encoded sample plain-text file
    ‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾

    Markus Kuhn [ˈmaʳkʊs kuːn] <http://www.cl.cam.ac.uk/~mgk25/> — 2002-07-25


    The ASCII compatible UTF-8 encoding used in this plain-text file
    is defined in Unicode, ISO 10646-1, and RFC 2279.


    Using Unicode/UTF-8, you can write in emails and source code things such as

    Mathematics and sciences:

    ∮ E⋅da = Q,  n → ∞, ∑ f(i) = ∏ g(i),      ⎧⎡⎛┌─────┐⎞⎤⎫
                                                ⎪⎢⎜│a²+b³ ⎟⎥⎪
    ∀x∈ℝ: ⌈x⌉ = −⌊−x⌋, α ∧ ¬β = ¬(¬α ∨ β),    ⎪⎢⎜│───── ⎟⎥⎪
                                                ⎪⎢⎜⎷ c₈   ⎟⎥⎪
    ℕ ⊆ ℕ₀ ⊂ ℤ ⊂ ℚ ⊂ ℝ ⊂ ℂ,                   ⎨⎢⎜       ⎟⎥⎬
                                                ⎪⎢⎜ ∞     ⎟⎥⎪
    ⊥ < a ≠ b ≡ c ≤ d ≪ ⊤ ⇒ (⟦A⟧ ⇔ ⟪B⟫),      ⎪⎢⎜ ⎲     ⎟⎥⎪
                                                ⎪⎢⎜ ⎳aⁱ-bⁱ⎟⎥⎪
    2H₂ + O₂ ⇌ 2H₂O, R = 4.7 kΩ, ⌀ 200 mm     ⎩⎣⎝i=1    ⎠⎦⎭

    Linguistics and dictionaries:

    ði ıntəˈnæʃənəl fəˈnɛtık əsoʊsiˈeıʃn
    Y [ˈʏpsilɔn], Yen [jɛn], Yoga [ˈjoːgɑ]

    APL:

    ((V⍳V)=⍳⍴V)/V←,V    ⌷←⍳→⍴∆∇⊃‾⍎⍕⌈

    Nicer typography in plain text files:

    ╔══════════════════════════════════════════╗
    ║                                          ║
    ║   • ‘single’ and “double” quotes         ║
    ║                                          ║
    ║   • Curly apostrophes: “We’ve been here” ║
    ║                                          ║
    ║   • Latin-1 apostrophe and accents: '´`  ║
    ║                                          ║
    ║   • ‚deutsche‘ „Anführungszeichen“       ║
    ║                                          ║
    ║   • †, ‡, ‰, •, 3–4, —, −5/+5, ™, …      ║
    ║                                          ║
    ║   • ASCII safety test: 1lI|, 0OD, 8B     ║
    ║                      ╭─────────╮         ║
    ║   • the euro symbol: │ 14.95 € │         ║
    ║                      ╰─────────╯         ║
    ╚══════════════════════════════════════════╝

    Combining characters:

    STARGΛ̊TE SG-1, a = v̇ = r̈, a⃑ ⊥ b⃑

    Greek (in Polytonic):

    The Greek anthem:

    Σὲ γνωρίζω ἀπὸ τὴν κόψη
    τοῦ σπαθιοῦ τὴν τρομερή,
    σὲ γνωρίζω ἀπὸ τὴν ὄψη
    ποὺ μὲ βία μετράει τὴ γῆ.

    ᾿Απ᾿ τὰ κόκκαλα βγαλμένη
    τῶν ῾Ελλήνων τὰ ἱερά
    καὶ σὰν πρῶτα ἀνδρειωμένη
    χαῖρε, ὦ χαῖρε, ᾿Ελευθεριά!

    From a speech of Demosthenes in the 4th century BC:

    Οὐχὶ ταὐτὰ παρίσταταί μοι γιγνώσκειν, ὦ ἄνδρες ᾿Αθηναῖοι,
    ὅταν τ᾿ εἰς τὰ πράγματα ἀποβλέψω καὶ ὅταν πρὸς τοὺς
    λόγους οὓς ἀκούω· τοὺς μὲν γὰρ λόγους περὶ τοῦ
    τιμωρήσασθαι Φίλιππον ὁρῶ γιγνομένους, τὰ δὲ πράγματ᾿
    εἰς τοῦτο προήκοντα,  ὥσθ᾿ ὅπως μὴ πεισόμεθ᾿ αὐτοὶ
    πρότερον κακῶς σκέψασθαι δέον. οὐδέν οὖν ἄλλο μοι δοκοῦσιν
    οἱ τὰ τοιαῦτα λέγοντες ἢ τὴν ὑπόθεσιν, περὶ ἧς βουλεύεσθαι,
    οὐχὶ τὴν οὖσαν παριστάντες ὑμῖν ἁμαρτάνειν. ἐγὼ δέ, ὅτι μέν
    ποτ᾿ ἐξῆν τῇ πόλει καὶ τὰ αὑτῆς ἔχειν ἀσφαλῶς καὶ Φίλιππον
    τιμωρήσασθαι, καὶ μάλ᾿ ἀκριβῶς οἶδα· ἐπ᾿ ἐμοῦ γάρ, οὐ πάλαι
    γέγονεν ταῦτ᾿ ἀμφότερα· νῦν μέντοι πέπεισμαι τοῦθ᾿ ἱκανὸν
    προλαβεῖν ἡμῖν εἶναι τὴν πρώτην, ὅπως τοὺς συμμάχους
    σώσομεν. ἐὰν γὰρ τοῦτο βεβαίως ὑπάρξῃ, τότε καὶ περὶ τοῦ
    τίνα τιμωρήσεταί τις καὶ ὃν τρόπον ἐξέσται σκοπεῖν· πρὶν δὲ
    τὴν ἀρχὴν ὀρθῶς ὑποθέσθαι, μάταιον ἡγοῦμαι περὶ τῆς
    τελευτῆς ὁντινοῦν ποιεῖσθαι λόγον.

    Δημοσθένους, Γ´ ᾿Ολυνθιακὸς

    Georgian:

    From a Unicode conference invitation:

    გთხოვთ ახლავე გაიაროთ რეგისტრაცია Unicode-ის მეათე საერთაშორისო
    კონფერენციაზე დასასწრებად, რომელიც გაიმართება 10-12 მარტს,
    ქ. მაინცში, გერმანიაში. კონფერენცია შეჰკრებს ერთად მსოფლიოს
    ექსპერტებს ისეთ დარგებში როგორიცაა ინტერნეტი და Unicode-ი,
    ინტერნაციონალიზაცია და ლოკალიზაცია, Unicode-ის გამოყენება
    ოპერაციულ სისტემებსა, და გამოყენებით პროგრამებში, შრიფტებში,
    ტექსტების დამუშავებასა და მრავალენოვან კომპიუტერულ სისტემებში.

    Russian:

    From a Unicode conference invitation:

    Зарегистрируйтесь сейчас на Десятую Международную Конференцию по
    Unicode, которая состоится 10-12 марта 1997 года в Майнце в Германии.
    Конференция соберет широкий круг экспертов по  вопросам глобального
    Интернета и Unicode, локализации и интернационализации, воплощению и
    применению Unicode в различных операционных системах и программных
    приложениях, шрифтах, верстке и многоязычных компьютерных системах.

    Thai (UCS Level 2):

    Excerpt from a poetry on The Romance of The Three Kingdoms (a Chinese
    classic 'San Gua'):

    [----------------------------|------------------------]
        ๏ แผ่นดินฮั่นเสื่อมโทรมแสนสังเวช  พระปกเกศกองบู๊กู้ขึ้นใหม่
    สิบสองกษัตริย์ก่อนหน้าแลถัดไป       สององค์ไซร้โง่เขลาเบาปัญญา
        ทรงนับถือขันทีเป็นที่พึ่ง           บ้านเมืองจึงวิปริตเป็นนักหนา
    โฮจิ๋นเรียกทัพทั่วหัวเมืองมา         หมายจะฆ่ามดชั่วตัวสำคัญ
        เหมือนขับไสไล่เสือจากเคหา      รับหมาป่าเข้ามาเลยอาสัญ
    ฝ่ายอ้องอุ้นยุแยกให้แตกกัน          ใช้สาวนั้นเป็นชนวนชื่นชวนใจ
        พลันลิฉุยกุยกีกลับก่อเหตุ          ช่างอาเพศจริงหนาฟ้าร้องไห้
    ต้องรบราฆ่าฟันจนบรรลัย           ฤๅหาใครค้ำชูกู้บรรลังก์ ฯ

    (The above is a two-column text. If combining characters are handled
    correctly, the lines of the second column should be aligned with the
    | character above.)

    Ethiopian:

    Proverbs in the Amharic language:

    ሰማይ አይታረስ ንጉሥ አይከሰስ።
    ብላ ካለኝ እንደአባቴ በቆመጠኝ።
    ጌጥ ያለቤቱ ቁምጥና ነው።
    ደሀ በሕልሙ ቅቤ ባይጠጣ ንጣት በገደለው።
    የአፍ ወለምታ በቅቤ አይታሽም።
    አይጥ በበላ ዳዋ ተመታ።
    ሲተረጉሙ ይደረግሙ።
    ቀስ በቀስ፥ ዕንቁላል በእግሩ ይሄዳል።
    ድር ቢያብር አንበሳ ያስር።
    ሰው እንደቤቱ እንጅ እንደ ጉረቤቱ አይተዳደርም።
    እግዜር የከፈተውን ጉሮሮ ሳይዘጋው አይድርም።
    የጎረቤት ሌባ፥ ቢያዩት ይስቅ ባያዩት ያጠልቅ።
    ሥራ ከመፍታት ልጄን ላፋታት።
    ዓባይ ማደሪያ የለው፥ ግንድ ይዞ ይዞራል።
    የእስላም አገሩ መካ የአሞራ አገሩ ዋርካ።
    ተንጋሎ ቢተፉ ተመልሶ ባፉ።
    ወዳጅህ ማር ቢሆን ጨርስህ አትላሰው።
    እግርህን በፍራሽህ ልክ ዘርጋ።

    Runes:

    ᚻᛖ ᚳᚹᚫᚦ ᚦᚫᛏ ᚻᛖ ᛒᚢᛞᛖ ᚩᚾ ᚦᚫᛗ ᛚᚪᚾᛞᛖ ᚾᚩᚱᚦᚹᛖᚪᚱᛞᚢᛗ ᚹᛁᚦ ᚦᚪ ᚹᛖᛥᚫ

    (Old English, which transcribed into Latin reads 'He cwaeth that he
    bude thaem lande northweardum with tha Westsae.' and means 'He said
    that he lived in the northern land near the Western Sea.')

    Braille:

    ⡌⠁⠧⠑ ⠼⠁⠒  ⡍⠜⠇⠑⠹⠰⠎ ⡣⠕⠌

    ⡍⠜⠇⠑⠹ ⠺⠁⠎ ⠙⠑⠁⠙⠒ ⠞⠕ ⠃⠑⠛⠔ ⠺⠊⠹⠲ ⡹⠻⠑ ⠊⠎ ⠝⠕ ⠙⠳⠃⠞
    ⠱⠁⠞⠑⠧⠻ ⠁⠃⠳⠞ ⠹⠁⠞⠲ ⡹⠑ ⠗⠑⠛⠊⠌⠻ ⠕⠋ ⠙⠊⠎ ⠃⠥⠗⠊⠁⠇ ⠺⠁⠎
    ⠎⠊⠛⠝⠫ ⠃⠹ ⠹⠑ ⠊⠇⠻⠛⠹⠍⠁⠝⠂ ⠹⠑ ⠊⠇⠻⠅⠂ ⠹⠑ ⠥⠝⠙⠻⠞⠁⠅⠻⠂
    ⠁⠝⠙ ⠹⠑ ⠡⠊⠑⠋ ⠍⠳⠗⠝⠻⠲ ⡎⠊⠗⠕⠕⠛⠑ ⠎⠊⠛⠝⠫ ⠊⠞⠲ ⡁⠝⠙
    ⡎⠊⠗⠕⠕⠛⠑⠰⠎ ⠝⠁⠍⠑ ⠺⠁⠎ ⠛⠕⠕⠙ ⠥⠏⠕⠝ ⠰⡡⠁⠝⠛⠑⠂ ⠋⠕⠗ ⠁⠝⠹⠹⠔⠛ ⠙⠑
    ⠡⠕⠎⠑ ⠞⠕ ⠏⠥⠞ ⠙⠊⠎ ⠙⠁⠝⠙ ⠞⠕⠲

    ⡕⠇⠙ ⡍⠜⠇⠑⠹ ⠺⠁⠎ ⠁⠎ ⠙⠑⠁⠙ ⠁⠎ ⠁ ⠙⠕⠕⠗⠤⠝⠁⠊⠇⠲

    ⡍⠔⠙⠖ ⡊ ⠙⠕⠝⠰⠞ ⠍⠑⠁⠝ ⠞⠕ ⠎⠁⠹ ⠹⠁⠞ ⡊ ⠅⠝⠪⠂ ⠕⠋ ⠍⠹
    ⠪⠝ ⠅⠝⠪⠇⠫⠛⠑⠂ ⠱⠁⠞ ⠹⠻⠑ ⠊⠎ ⠏⠜⠞⠊⠊⠥⠇⠜⠇⠹ ⠙⠑⠁⠙ ⠁⠃⠳⠞
    ⠁ ⠙⠕⠕⠗⠤⠝⠁⠊⠇⠲ ⡊ ⠍⠊⠣⠞ ⠙⠁⠧⠑ ⠃⠑⠲ ⠔⠊⠇⠔⠫⠂ ⠍⠹⠎⠑⠇⠋⠂ ⠞⠕
    ⠗⠑⠛⠜⠙ ⠁ ⠊⠕⠋⠋⠔⠤⠝⠁⠊⠇ ⠁⠎ ⠹⠑ ⠙⠑⠁⠙⠑⠌ ⠏⠊⠑⠊⠑ ⠕⠋ ⠊⠗⠕⠝⠍⠕⠝⠛⠻⠹
    ⠔ ⠹⠑ ⠞⠗⠁⠙⠑⠲ ⡃⠥⠞ ⠹⠑ ⠺⠊⠎⠙⠕⠍ ⠕⠋ ⠳⠗ ⠁⠝⠊⠑⠌⠕⠗⠎
    ⠊⠎ ⠔ ⠹⠑ ⠎⠊⠍⠊⠇⠑⠆ ⠁⠝⠙ ⠍⠹ ⠥⠝⠙⠁⠇⠇⠪⠫ ⠙⠁⠝⠙⠎
    ⠩⠁⠇⠇ ⠝⠕⠞ ⠙⠊⠌⠥⠗⠃ ⠊⠞⠂ ⠕⠗ ⠹⠑ ⡊⠳⠝⠞⠗⠹⠰⠎ ⠙⠕⠝⠑ ⠋⠕⠗⠲ ⡹⠳
    ⠺⠊⠇⠇ ⠹⠻⠑⠋⠕⠗⠑ ⠏⠻⠍⠊⠞ ⠍⠑ ⠞⠕ ⠗⠑⠏⠑⠁⠞⠂ ⠑⠍⠏⠙⠁⠞⠊⠊⠁⠇⠇⠹⠂ ⠹⠁⠞
    ⡍⠜⠇⠑⠹ ⠺⠁⠎ ⠁⠎ ⠙⠑⠁⠙ ⠁⠎ ⠁ ⠙⠕⠕⠗⠤⠝⠁⠊⠇⠲

    (The first couple of paragraphs of "A Christmas Carol" by Dickens)

    Compact font selection example text:

    ABCDEFGHIJKLMNOPQRSTUVWXYZ /0123456789
    abcdefghijklmnopqrstuvwxyz £©µÀÆÖÞßéöÿ
    –—‘“”„†•…‰™œŠŸž€ ΑΒΓΔΩαβγδω АБВГДабвгд
    ∀∂∈ℝ∧∪≡∞ ↑↗↨↻⇣ ┐┼╔╘░►☺♀ ﬁ�⑀₂ἠḂӥẄɐː⍎אԱა

    Greetings in various languages:

    Hello world, Καλημέρα κόσμε, コンニチハ

    Box drawing alignment tests:                                          █
                                                                        ▉
    ╔══╦══╗  ┌──┬──┐  ╭──┬──╮  ╭──┬──╮  ┏━━┳━━┓  ┎┒┏┑   ╷  ╻ ┏┯┓ ┌┰┐    ▊ ╱╲╱╲╳╳╳
    ║┌─╨─┐║  │╔═╧═╗│  │╒═╪═╕│  │╓─╁─╖│  ┃┌─╂─┐┃  ┗╃╄┙  ╶┼╴╺╋╸┠┼┨ ┝╋┥    ▋ ╲╱╲╱╳╳╳
    ║│╲ ╱│║  │║   ║│  ││ │ ││  │║ ┃ ║│  ┃│ ╿ │┃  ┍╅╆┓   ╵  ╹ ┗┷┛ └┸┘    ▌ ╱╲╱╲╳╳╳
    ╠╡ ╳ ╞╣  ├╢   ╟┤  ├┼─┼─┼┤  ├╫─╂─╫┤  ┣┿╾┼╼┿┫  ┕┛┖┚     ┌┄┄┐ ╎ ┏┅┅┓ ┋ ▍ ╲╱╲╱╳╳╳
    ║│╱ ╲│║  │║   ║│  ││ │ ││  │║ ┃ ║│  ┃│ ╽ │┃  ░░▒▒▓▓██ ┊  ┆ ╎ ╏  ┇ ┋ ▎
    ║└─╥─┘║  │╚═╤═╝│  │╘═╪═╛│  │╙─╀─╜│  ┃└─╂─┘┃  ░░▒▒▓▓██ ┊  ┆ ╎ ╏  ┇ ┋ ▏
    ╚══╩══╝  └──┴──┘  ╰──┴──╯  ╰──┴──╯  ┗━━┻━━┛  ▗▄▖▛▀▜   └╌╌┘ ╎ ┗╍╍┛ ┋  ▁▂▃▄▅▆▇█
                                                ▝▀▘▙▄▟

    </pre>
  </body>
</html>"#;

    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(utf8_content))
}

pub async fn gzip_handler(req: HttpRequest, config: web::Data<AppConfig>) -> Result<HttpResponse> {
    let mut request_info = extract_request_info(&req, None, &config.exclude_headers);
    fix_request_info_url(&req, &mut request_info);

    // Add gzipped flag for httpbin compatibility
    let mut response_data = serde_json::to_value(&request_info).unwrap();
    if let Some(obj) = response_data.as_object_mut() {
        obj.insert("gzipped".to_string(), serde_json::Value::Bool(true));
    }

    let json_data = serde_json::to_vec(&response_data).unwrap();

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&json_data).unwrap();
    let compressed_data = encoder.finish().unwrap();

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .append_header(("Content-Encoding", "gzip"))
        .body(compressed_data))
}

pub async fn deflate_handler(
    req: HttpRequest,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse> {
    let mut request_info = extract_request_info(&req, None, &config.exclude_headers);
    fix_request_info_url(&req, &mut request_info);

    // Add deflated flag for httpbin compatibility
    let mut response_data = serde_json::to_value(&request_info).unwrap();
    if let Some(obj) = response_data.as_object_mut() {
        obj.insert("deflated".to_string(), serde_json::Value::Bool(true));
    }

    let json_data = serde_json::to_vec(&response_data).unwrap();

    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&json_data).unwrap();
    let compressed_data = encoder.finish().unwrap();

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .append_header(("Content-Encoding", "deflate"))
        .body(compressed_data))
}

pub async fn brotli_handler(
    req: HttpRequest,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse> {
    let mut request_info = extract_request_info(&req, None, &config.exclude_headers);
    fix_request_info_url(&req, &mut request_info);

    // Add brotli flag for httpbin compatibility
    let mut response_data = serde_json::to_value(&request_info).unwrap();
    if let Some(obj) = response_data.as_object_mut() {
        obj.insert("brotli".to_string(), serde_json::Value::Bool(true));
    }

    let json_data = serde_json::to_vec(&response_data).unwrap();

    let mut compressed_data = Vec::new();
    let mut writer = brotli::CompressorWriter::new(&mut compressed_data, 4096, 6, 22);
    writer.write_all(&json_data).unwrap();
    drop(writer);

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .append_header(("Content-Encoding", "br"))
        .body(compressed_data))
}

pub async fn zstd_handler(req: HttpRequest, config: web::Data<AppConfig>) -> Result<HttpResponse> {
    let mut request_info = extract_request_info(&req, None, &config.exclude_headers);
    fix_request_info_url(&req, &mut request_info);

    // Add zstd flag for httpbin compatibility
    let mut response_data = serde_json::to_value(&request_info).unwrap();
    if let Some(obj) = response_data.as_object_mut() {
        obj.insert("zstd".to_string(), serde_json::Value::Bool(true));
    }

    let json_data = serde_json::to_vec(&response_data).unwrap();

    let mut compressed_data = Vec::new();
    {
        let mut writer = ZstdEncoder::new(&mut compressed_data, 3).unwrap();
        writer.write_all(&json_data).unwrap();
        writer.finish().unwrap();
    }

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .append_header(("Content-Encoding", "zstd"))
        .body(compressed_data))
}

/// Compress bytes with the given content-encoding: gzip, deflate, br, or zstd.
fn compress_bytes(data: &[u8], encoding: &str) -> Vec<u8> {
    match encoding {
        "gzip" => {
            let mut e = GzEncoder::new(Vec::new(), Compression::default());
            let _ = e.write_all(data);
            e.finish().unwrap_or_default()
        }
        "deflate" => {
            let mut e = DeflateEncoder::new(Vec::new(), Compression::default());
            let _ = e.write_all(data);
            e.finish().unwrap_or_default()
        }
        "br" => {
            let mut out = Vec::new();
            {
                let mut w = brotli::CompressorWriter::new(&mut out, 4096, 6, 22);
                let _ = w.write_all(data);
            }
            out
        }
        "zstd" => {
            let mut out = Vec::new();
            if let Ok(mut w) = ZstdEncoder::new(&mut out, 3) {
                let _ = w.write_all(data);
                let _ = w.finish();
            }
            out
        }
        _ => data.to_vec(),
    }
}

/// POST /gzip|/deflate|/brotli|/zstd -> echo the request body compressed with
/// the matching Content-Encoding (httpbin #618).
pub async fn compress_post_handler(req: HttpRequest, body: web::Bytes) -> Result<HttpResponse> {
    let encoding = match req.uri().path() {
        "/gzip" => "gzip",
        "/deflate" => "deflate",
        "/brotli" => "br",
        "/zstd" => "zstd",
        _ => "gzip",
    };
    let content_type = req
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("text/plain")
        .to_string();
    let compressed = compress_bytes(&body, encoding);
    Ok(HttpResponse::Ok()
        .content_type(content_type)
        .append_header(("Content-Encoding", encoding))
        .body(compressed))
}

/// GET /encoding/iso-8859-1 - Latin-1 sample, companion to /encoding/utf8 (httpbin #427).
pub async fn iso_8859_1_handler(_req: HttpRequest) -> Result<HttpResponse> {
    let content = "\
<!DOCTYPE html>\n\
<html>\n  <head>\n    <meta charset=\"ISO-8859-1\">\n    <title>ISO-8859-1 Test</title>\n  </head>\n  <body>\n    <h1>Latin-1 Demo</h1>\n    <p>caf\u{e9} r\u{e9}sum\u{e9} na\u{ef}ve \u{fc}ber a\u{f1}o fa\u{e7}ade</p>\n    <p>\u{a3}100 \u{a5}1000 50\u{a2}</p>\n  </body>\n</html>";
    // Encode as ISO-8859-1 (Latin-1): each char in U+0000..U+00FF maps to its low byte.
    let bytes: Vec<u8> = content.chars().map(|c| c as u8).collect();
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=iso-8859-1")
        .body(bytes))
}
