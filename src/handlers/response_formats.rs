use super::*;
use flate2::{write::GzEncoder, write::DeflateEncoder, Compression};
use std::io::Write;
use crate::handlers::utils::get_static_path;

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

pub async fn robots_txt_handler(_req: HttpRequest) -> Result<HttpResponse> {
    let robots_content = "User-agent: *\nDisallow: /deny\n";
    
    Ok(HttpResponse::Ok()
        .content_type("text/plain")
        .body(robots_content))
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

pub async fn deflate_handler(req: HttpRequest, config: web::Data<AppConfig>) -> Result<HttpResponse> {
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

pub async fn brotli_handler(req: HttpRequest, config: web::Data<AppConfig>) -> Result<HttpResponse> {
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

pub async fn root_handler(req: HttpRequest, config: web::Data<AppConfig>) -> Result<HttpResponse> {
    // Check the Accept header to determine response format
    let accept_header = req
        .headers()
        .get("accept")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    
    // If Accept header contains "html", serve the HTML page
    if accept_header.to_lowercase().contains("html") {
        // Try to serve the static index.html file
        let static_path = get_static_path();
        let index_path = static_path.join("index.html");
        
        match std::fs::read_to_string(&index_path) {
            Ok(html_content) => {
                Ok(HttpResponse::Ok()
                    .content_type("text/html; charset=utf-8")
                    .body(html_content))
            }
            Err(_) => {
                // Fallback to a helpful HTML response if index.html is not found
                let version = option_env!("CARGO_PKG_VERSION").unwrap_or("unknown");
                let fallback_html = format!(r#"<!DOCTYPE html>
<html>
<head>
    <title>HTTPCan v{}</title>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; margin: 40px; line-height: 1.6; color: #333; }}
        .container {{ max-width: 800px; margin: 0 auto; }}
        h1 {{ color: #2c3e50; }}
        .version {{ color: #7f8c8d; font-size: 0.9em; }}
        .message {{ background: #f8f9fa; padding: 20px; border-radius: 8px; border-left: 4px solid #3498db; margin: 20px 0; }}
        .code {{ background: #f4f4f4; padding: 2px 6px; border-radius: 3px; font-family: 'Monaco', 'Consolas', monospace; }}
        a {{ color: #3498db; text-decoration: none; }}
        a:hover {{ text-decoration: underline; }}
    </style>
</head>
<body>
    <div class="container">
        <h1>HTTPCan <span class="version">v{}</span></h1>
        <p>A simple, high‑performance HTTP request & response service built with Rust and Actix Web. Fully compatible with [httpbin.org](https://httpbin.org), with modern streaming and AI‑friendly enhancements.</p>
        
        <div class="message">
            <h3>Setup Required</h3>
            <p><strong>index.html not found.</strong> To get the full web interface:</p>
            <ol>
                <li>Download <span class="code">index.html</span> from <a href="https://httpcan.org" target="_blank">https://httpcan.org</a></li>
                <li>Create a <span class="code">static</span> directory in the directory where the httpcan binary file is located</li>
                <li>Place the downloaded <span class="code">index.html</span> into that directory</li>
            </ol>
        </div>
        
        <h3>API Documentation</h3>
        <p>Visit <a href="/openapi.json">/openapi.json</a> for API documentation.</p>
        
        <h3>Quick Test</h3>
        <p>Try these endpoints:</p>
        <ul>
            <li><a href="/get">/get</a> - Test GET requests</li>
            <li><a href="/post">/post</a> - Test POST requests</li>
            <li><a href="/headers">/headers</a> - View request headers</li>
            <li><a href="/ip">/ip</a> - Get your IP address</li>
            <li><a href="/sse">/sse</a> - Server-Sent Events stream</li>
        </ul>
    </div>
</body>
</html>"#, version, version);
                Ok(HttpResponse::Ok()
                    .content_type("text/html; charset=utf-8")
                    .body(fallback_html))
            }
        }
    } else {
        // If Accept header doesn't contain "html", return OpenAPI specification
        // Use the same logic as /openapi.json endpoint
        let static_path = get_static_path();
        let openapi_path = static_path.join("openapi.json");
        
        // Read the base OpenAPI specification
        let base_openapi = match std::fs::read_to_string(&openapi_path) {
            Ok(content) => content,
            Err(_) => {
                // Return helpful information when openapi.json is not found
                return Ok(HttpResponse::NotFound().json(json!({
                    "info": {
                        "title": "HTTPCan",
                        "version": option_env!("CARGO_PKG_VERSION").unwrap_or("unknown"),
                        "description": "A simple, high‑performance HTTP request & response service built with Rust and Actix Web. Fully compatible with [httpbin.org](https://httpbin.org), with modern streaming and AI‑friendly enhancements."
                    },
                    "error": "OpenAPI specification not found",
                    "message": "Please download openapi.json from https://httpcan.org. Then create a static directory in the directory where the httpcan binary file is located, and place the downloaded openapi.json into that directory."
                })));
            }
        };
        
        // Parse the base OpenAPI JSON
        let mut openapi: serde_json::Value = match serde_json::from_str(&base_openapi) {
            Ok(spec) => spec,
            Err(_) => {
                return Ok(HttpResponse::InternalServerError().json(json!({
                    "error": "Failed to parse OpenAPI specification"
                })));
            }
        };
        
        // Handle servers array based on configuration
        if config.add_current_server {
            // Get current server information from request
            let connection_info = req.connection_info();
            let scheme = connection_info.scheme();
            let host = connection_info.host();
            let current_server_url = format!("{}://{}", scheme, host);
            
            // Get existing servers array from the OpenAPI spec
            let mut servers_array = Vec::new();
            
            // Add current server as the first element
            servers_array.push(json!({
                "url": current_server_url,
                "description": "Current server"
            }));
            
            // Add existing servers from the original OpenAPI spec
            if let Some(existing_servers) = openapi.get("servers").and_then(|s| s.as_array()) {
                for server in existing_servers {
                    // Skip if it's the same as current server URL to avoid duplicates
                    if let Some(url) = server.get("url").and_then(|u| u.as_str()) {
                        if url != current_server_url {
                            servers_array.push(server.clone());
                        }
                    }
                }
            }
            
            // Update the servers field
            if let Some(obj) = openapi.as_object_mut() {
                obj.insert("servers".to_string(), json!(servers_array));
            }
        }
        // If add_current_server is false, keep the original servers array unchanged
        
        Ok(HttpResponse::Ok()
            .content_type("application/json")
            .json(openapi))
    }
}
