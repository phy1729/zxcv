use anyhow::bail;
use anyhow::Context;
use serde::Deserialize;
use ureq::Agent;
use url::Url;

use crate::html;
use crate::Content;
use crate::Post;
use crate::PostThread;
use crate::TextType;

const API_BASE: &str = "https://api.stackexchange.com/2.3/";
const FILTER: &str = "!T*hPNRA69ofM1izkPP";

// curl --compressed 'https://api.stackexchange.com/2.3/sites?pagesize=10000'
// jq -r '.items[] | "        " + ([.site_url] + .aliases | map(ltrimstr("https://") | "\"" + . + "\"") | join (" | ")) + " => Some(\"" + .api_site_parameter + "\"),"'
#[allow(clippy::too_many_lines)]
#[rustfmt::skip]
fn site_tag(hostname: &str) -> Option<&'static str> {
    match hostname {
        "stackoverflow.com" | "www.stackoverflow.com" | "facebook.stackoverflow.com" => Some("stackoverflow"),
        "serverfault.com" => Some("serverfault"),
        "superuser.com" => Some("superuser"),
        "meta.stackexchange.com" => Some("meta"),
        "webapps.stackexchange.com" | "nothingtoinstall.com" => Some("webapps"),
        "webapps.meta.stackexchange.com" | "meta.nothingtoinstall.com" | "meta.webapps.stackexchange.com" => Some("webapps.meta"),
        "gaming.stackexchange.com" | "arqade.com" | "thearqade.com" => Some("gaming"),
        "gaming.meta.stackexchange.com" | "meta.arqade.com" | "meta.thearqade.com" | "meta.gaming.stackexchange.com" => Some("gaming.meta"),
        "webmasters.stackexchange.com" | "webmaster.stackexchange.com" => Some("webmasters"),
        "webmasters.meta.stackexchange.com" | "meta.webmaster.stackexchange.com" | "meta.webmasters.stackexchange.com" => Some("webmasters.meta"),
        "cooking.stackexchange.com" | "seasonedadvice.com" => Some("cooking"),
        "cooking.meta.stackexchange.com" | "meta.seasonedadvice.com" | "meta.cooking.stackexchange.com" => Some("cooking.meta"),
        "gamedev.stackexchange.com" => Some("gamedev"),
        "gamedev.meta.stackexchange.com" | "meta.gamedev.stackexchange.com" => Some("gamedev.meta"),
        "photo.stackexchange.com" | "photography.stackexchange.com" | "photos.stackexchange.com" => Some("photo"),
        "photo.meta.stackexchange.com" | "meta.photography.stackexchange.com" | "meta.photos.stackexchange.com" | "meta.photo.stackexchange.com" => Some("photo.meta"),
        "stats.stackexchange.com" | "statistics.stackexchange.com" | "crossvalidated.com" => Some("stats"),
        "stats.meta.stackexchange.com" | "meta.statistics.stackexchange.com" | "meta.stats.stackexchange.com" => Some("stats.meta"),
        "math.stackexchange.com" | "maths.stackexchange.com" | "mathematics.stackexchange.com" => Some("math"),
        "math.meta.stackexchange.com" | "meta.math.stackexchange.com" => Some("math.meta"),
        "diy.stackexchange.com" => Some("diy"),
        "diy.meta.stackexchange.com" | "meta.diy.stackexchange.com" => Some("diy.meta"),
        "meta.superuser.com" => Some("meta.superuser"),
        "meta.serverfault.com" => Some("meta.serverfault"),
        "gis.stackexchange.com" => Some("gis"),
        "gis.meta.stackexchange.com" | "meta.gis.stackexchange.com" => Some("gis.meta"),
        "tex.stackexchange.com" => Some("tex"),
        "tex.meta.stackexchange.com" | "meta.tex.stackexchange.com" => Some("tex.meta"),
        "askubuntu.com" | "ubuntu.stackexchange.com" => Some("askubuntu"),
        "meta.askubuntu.com" | "meta.ubuntu.stackexchange.com" => Some("meta.askubuntu"),
        "money.stackexchange.com" | "basicallymoney.com" | "www.basicallymoney.com" => Some("money"),
        "money.meta.stackexchange.com" | "meta.money.stackexchange.com" => Some("money.meta"),
        "english.stackexchange.com" | "elu.stackexchange.com" => Some("english"),
        "english.meta.stackexchange.com" | "meta.english.stackexchange.com" => Some("english.meta"),
        "stackapps.com" => Some("stackapps"),
        "ux.stackexchange.com" | "ui.stackexchange.com" => Some("ux"),
        "ux.meta.stackexchange.com" | "meta.ui.stackexchange.com" | "meta.ux.stackexchange.com" => Some("ux.meta"),
        "unix.stackexchange.com" | "linux.stackexchange.com" => Some("unix"),
        "unix.meta.stackexchange.com" | "meta.linux.stackexchange.com" | "meta.unix.stackexchange.com" => Some("unix.meta"),
        "wordpress.stackexchange.com" => Some("wordpress"),
        "wordpress.meta.stackexchange.com" | "meta.wordpress.stackexchange.com" => Some("wordpress.meta"),
        "cstheory.stackexchange.com" => Some("cstheory"),
        "cstheory.meta.stackexchange.com" | "meta.cstheory.stackexchange.com" => Some("cstheory.meta"),
        "apple.stackexchange.com" | "askdifferent.com" => Some("apple"),
        "apple.meta.stackexchange.com" | "meta.apple.stackexchange.com" => Some("apple.meta"),
        "rpg.stackexchange.com" => Some("rpg"),
        "rpg.meta.stackexchange.com" | "meta.rpg.stackexchange.com" => Some("rpg.meta"),
        "bicycles.stackexchange.com" | "bicycle.stackexchange.com" | "cycling.stackexchange.com" | "bikes.stackexchange.com" => Some("bicycles"),
        "bicycles.meta.stackexchange.com" | "meta.bicycle.stackexchange.com" | "meta.bicycles.stackexchange.com" => Some("bicycles.meta"),
        "softwareengineering.stackexchange.com" | "programmer.stackexchange.com" | "programmers.stackexchange.com" | "swe.stackexchange.com" => Some("softwareengineering"),
        "softwareengineering.meta.stackexchange.com" | "meta.programmers.stackexchange.com" | "meta.softwareengineering.stackexchange.com" | "meta.swe.stackexchange.com" => Some("softwareengineering.meta"),
        "electronics.stackexchange.com" | "chiphacker.com" | "www.chiphacker.com" => Some("electronics"),
        "electronics.meta.stackexchange.com" | "meta.electronics.stackexchange.com" => Some("electronics.meta"),
        "android.stackexchange.com" => Some("android"),
        "android.meta.stackexchange.com" | "meta.android.stackexchange.com" => Some("android.meta"),
        "boardgames.stackexchange.com" | "boardgame.stackexchange.com" => Some("boardgames"),
        "boardgames.meta.stackexchange.com" | "meta.boardgames.stackexchange.com" => Some("boardgames.meta"),
        "physics.stackexchange.com" => Some("physics"),
        "physics.meta.stackexchange.com" | "meta.physics.stackexchange.com" => Some("physics.meta"),
        "homebrew.stackexchange.com" | "homebrewing.stackexchange.com" | "brewadvice.com" => Some("homebrew"),
        "homebrew.meta.stackexchange.com" | "meta.homebrewing.stackexchange.com" | "meta.homebrew.stackexchange.com" => Some("homebrew.meta"),
        "security.stackexchange.com" | "itsecurity.stackexchange.com" => Some("security"),
        "security.meta.stackexchange.com" | "meta.itsecurity.stackexchange.com" | "meta.security.stackexchange.com" => Some("security.meta"),
        "writing.stackexchange.com" | "writer.stackexchange.com" | "writers.stackexchange.com" => Some("writing"),
        "writing.meta.stackexchange.com" | "meta.writing.stackexchange.com" | "meta.writers.stackexchange.com" | "writers.meta.stackexchange.com" => Some("writing.meta"),
        "video.stackexchange.com" | "avp.stackexchange.com" => Some("video"),
        "video.meta.stackexchange.com" | "meta.avp.stackexchange.com" | "meta.video.stackexchange.com" => Some("video.meta"),
        "graphicdesign.stackexchange.com" | "graphicsdesign.stackexchange.com" | "graphicdesigns.stackexchange.com" => Some("graphicdesign"),
        "graphicdesign.meta.stackexchange.com" | "meta.graphicdesign.stackexchange.com" => Some("graphicdesign.meta"),
        "dba.stackexchange.com" => Some("dba"),
        "dba.meta.stackexchange.com" | "meta.dba.stackexchange.com" => Some("dba.meta"),
        "scifi.stackexchange.com" | "sciencefiction.stackexchange.com" | "fantasy.stackexchange.com" => Some("scifi"),
        "scifi.meta.stackexchange.com" | "meta.scifi.stackexchange.com" => Some("scifi.meta"),
        "codereview.stackexchange.com" => Some("codereview"),
        "codereview.meta.stackexchange.com" | "meta.codereview.stackexchange.com" => Some("codereview.meta"),
        "codegolf.stackexchange.com" => Some("codegolf"),
        "codegolf.meta.stackexchange.com" | "meta.codegolf.stackexchange.com" => Some("codegolf.meta"),
        "quant.stackexchange.com" => Some("quant"),
        "quant.meta.stackexchange.com" | "meta.quant.stackexchange.com" => Some("quant.meta"),
        "pm.stackexchange.com" => Some("pm"),
        "pm.meta.stackexchange.com" | "meta.pm.stackexchange.com" => Some("pm.meta"),
        "skeptics.stackexchange.com" | "skeptic.stackexchange.com" | "skepticexchange.com" => Some("skeptics"),
        "skeptics.meta.stackexchange.com" | "meta.skeptics.stackexchange.com" => Some("skeptics.meta"),
        "fitness.stackexchange.com" => Some("fitness"),
        "fitness.meta.stackexchange.com" | "meta.fitness.stackexchange.com" => Some("fitness.meta"),
        "drupal.stackexchange.com" => Some("drupal"),
        "drupal.meta.stackexchange.com" | "meta.drupal.stackexchange.com" => Some("drupal.meta"),
        "mechanics.stackexchange.com" | "garage.stackexchange.com" => Some("mechanics"),
        "mechanics.meta.stackexchange.com" | "meta.garage.stackexchange.com" | "meta.mechanics.stackexchange.com" => Some("mechanics.meta"),
        "parenting.stackexchange.com" => Some("parenting"),
        "parenting.meta.stackexchange.com" | "meta.parenting.stackexchange.com" => Some("parenting.meta"),
        "sharepoint.stackexchange.com" | "sharepointoverflow.com" | "www.sharepointoverflow.com" => Some("sharepoint"),
        "sharepoint.meta.stackexchange.com" | "meta.sharepoint.stackexchange.com" => Some("sharepoint.meta"),
        "music.stackexchange.com" | "guitars.stackexchange.com" | "guitar.stackexchange.com" => Some("music"),
        "music.meta.stackexchange.com" | "meta.music.stackexchange.com" => Some("music.meta"),
        "sqa.stackexchange.com" => Some("sqa"),
        "sqa.meta.stackexchange.com" | "meta.sqa.stackexchange.com" => Some("sqa.meta"),
        "judaism.stackexchange.com" | "mi.yodeya.com" | "yodeya.com" | "yodeya.stackexchange.com" | "miyodeya.com" => Some("judaism"),
        "judaism.meta.stackexchange.com" | "meta.judaism.stackexchange.com" => Some("judaism.meta"),
        "german.stackexchange.com" | "deutsch.stackexchange.com" => Some("german"),
        "german.meta.stackexchange.com" | "meta.german.stackexchange.com" => Some("german.meta"),
        "japanese.stackexchange.com" => Some("japanese"),
        "japanese.meta.stackexchange.com" | "meta.japanese.stackexchange.com" => Some("japanese.meta"),
        "philosophy.stackexchange.com" => Some("philosophy"),
        "philosophy.meta.stackexchange.com" | "meta.philosophy.stackexchange.com" => Some("philosophy.meta"),
        "gardening.stackexchange.com" | "landscaping.stackexchange.com" => Some("gardening"),
        "gardening.meta.stackexchange.com" | "meta.gardening.stackexchange.com" => Some("gardening.meta"),
        "travel.stackexchange.com" => Some("travel"),
        "travel.meta.stackexchange.com" | "meta.travel.stackexchange.com" => Some("travel.meta"),
        "crypto.stackexchange.com" | "cryptography.stackexchange.com" => Some("crypto"),
        "crypto.meta.stackexchange.com" | "meta.cryptography.stackexchange.com" | "meta.crypto.stackexchange.com" => Some("crypto.meta"),
        "dsp.stackexchange.com" | "signals.stackexchange.com" => Some("dsp"),
        "dsp.meta.stackexchange.com" | "meta.dsp.stackexchange.com" => Some("dsp.meta"),
        "french.stackexchange.com" => Some("french"),
        "french.meta.stackexchange.com" | "meta.french.stackexchange.com" => Some("french.meta"),
        "christianity.stackexchange.com" => Some("christianity"),
        "christianity.meta.stackexchange.com" | "meta.christianity.stackexchange.com" => Some("christianity.meta"),
        "bitcoin.stackexchange.com" => Some("bitcoin"),
        "bitcoin.meta.stackexchange.com" | "meta.bitcoin.stackexchange.com" => Some("bitcoin.meta"),
        "linguistics.stackexchange.com" | "linguist.stackexchange.com" => Some("linguistics"),
        "linguistics.meta.stackexchange.com" | "meta.linguistics.stackexchange.com" => Some("linguistics.meta"),
        "hermeneutics.stackexchange.com" => Some("hermeneutics"),
        "hermeneutics.meta.stackexchange.com" | "meta.hermeneutics.stackexchange.com" => Some("hermeneutics.meta"),
        "history.stackexchange.com" => Some("history"),
        "history.meta.stackexchange.com" | "meta.history.stackexchange.com" => Some("history.meta"),
        "bricks.stackexchange.com" => Some("bricks"),
        "bricks.meta.stackexchange.com" | "meta.bricks.stackexchange.com" => Some("bricks.meta"),
        "spanish.stackexchange.com" | "espanol.stackexchange.com" => Some("spanish"),
        "spanish.meta.stackexchange.com" | "meta.spanish.stackexchange.com" => Some("spanish.meta"),
        "scicomp.stackexchange.com" => Some("scicomp"),
        "scicomp.meta.stackexchange.com" | "meta.scicomp.stackexchange.com" => Some("scicomp.meta"),
        "movies.stackexchange.com" | "tv.stackexchange.com" => Some("movies"),
        "movies.meta.stackexchange.com" | "meta.tv.stackexchange.com" | "meta.movies.stackexchange.com" => Some("movies.meta"),
        "chinese.stackexchange.com" => Some("chinese"),
        "chinese.meta.stackexchange.com" | "meta.chinese.stackexchange.com" => Some("chinese.meta"),
        "biology.stackexchange.com" => Some("biology"),
        "biology.meta.stackexchange.com" | "meta.biology.stackexchange.com" => Some("biology.meta"),
        "poker.stackexchange.com" => Some("poker"),
        "poker.meta.stackexchange.com" | "meta.poker.stackexchange.com" => Some("poker.meta"),
        "mathematica.stackexchange.com" => Some("mathematica"),
        "mathematica.meta.stackexchange.com" | "meta.mathematica.stackexchange.com" => Some("mathematica.meta"),
        "psychology.stackexchange.com" | "cogsci.stackexchange.com" => Some("psychology"),
        "psychology.meta.stackexchange.com" | "meta.cogsci.stackexchange.com" | "cogsci.meta.stackexchange.com" => Some("psychology.meta"),
        "outdoors.stackexchange.com" => Some("outdoors"),
        "outdoors.meta.stackexchange.com" | "meta.outdoors.stackexchange.com" => Some("outdoors.meta"),
        "martialarts.stackexchange.com" => Some("martialarts"),
        "martialarts.meta.stackexchange.com" | "meta.martialarts.stackexchange.com" => Some("martialarts.meta"),
        "sports.stackexchange.com" => Some("sports"),
        "sports.meta.stackexchange.com" | "meta.sports.stackexchange.com" => Some("sports.meta"),
        "academia.stackexchange.com" | "academics.stackexchange.com" => Some("academia"),
        "academia.meta.stackexchange.com" | "meta.academia.stackexchange.com" => Some("academia.meta"),
        "cs.stackexchange.com" | "computerscience.stackexchange.com" => Some("cs"),
        "cs.meta.stackexchange.com" | "meta.cs.stackexchange.com" => Some("cs.meta"),
        "workplace.stackexchange.com" => Some("workplace"),
        "workplace.meta.stackexchange.com" | "meta.workplace.stackexchange.com" => Some("workplace.meta"),
        "chemistry.stackexchange.com" => Some("chemistry"),
        "chemistry.meta.stackexchange.com" | "meta.chemistry.stackexchange.com" => Some("chemistry.meta"),
        "chess.stackexchange.com" => Some("chess"),
        "chess.meta.stackexchange.com" | "meta.chess.stackexchange.com" => Some("chess.meta"),
        "raspberrypi.stackexchange.com" => Some("raspberrypi"),
        "raspberrypi.meta.stackexchange.com" | "meta.raspberrypi.stackexchange.com" => Some("raspberrypi.meta"),
        "russian.stackexchange.com" => Some("russian"),
        "russian.meta.stackexchange.com" | "meta.russian.stackexchange.com" => Some("russian.meta"),
        "islam.stackexchange.com" => Some("islam"),
        "islam.meta.stackexchange.com" | "meta.islam.stackexchange.com" => Some("islam.meta"),
        "salesforce.stackexchange.com" => Some("salesforce"),
        "salesforce.meta.stackexchange.com" | "meta.salesforce.stackexchange.com" => Some("salesforce.meta"),
        "patents.stackexchange.com" | "askpatents.com" | "askpatents.stackexchange.com" => Some("patents"),
        "patents.meta.stackexchange.com" | "meta.askpatents.com" | "meta.askpatents.stackexchange.com" | "meta.patents.stackexchange.com" => Some("patents.meta"),
        "genealogy.stackexchange.com" => Some("genealogy"),
        "genealogy.meta.stackexchange.com" | "meta.genealogy.stackexchange.com" => Some("genealogy.meta"),
        "robotics.stackexchange.com" => Some("robotics"),
        "robotics.meta.stackexchange.com" | "meta.robotics.stackexchange.com" => Some("robotics.meta"),
        "expressionengine.stackexchange.com" => Some("expressionengine"),
        "expressionengine.meta.stackexchange.com" | "meta.expressionengine.stackexchange.com" => Some("expressionengine.meta"),
        "politics.stackexchange.com" => Some("politics"),
        "politics.meta.stackexchange.com" | "meta.politics.stackexchange.com" => Some("politics.meta"),
        "anime.stackexchange.com" => Some("anime"),
        "anime.meta.stackexchange.com" | "meta.anime.stackexchange.com" => Some("anime.meta"),
        "magento.stackexchange.com" => Some("magento"),
        "magento.meta.stackexchange.com" | "meta.magento.stackexchange.com" => Some("magento.meta"),
        "ell.stackexchange.com" => Some("ell"),
        "ell.meta.stackexchange.com" | "meta.ell.stackexchange.com" => Some("ell.meta"),
        "sustainability.stackexchange.com" => Some("sustainability"),
        "sustainability.meta.stackexchange.com" | "meta.sustainability.stackexchange.com" => Some("sustainability.meta"),
        "tridion.stackexchange.com" => Some("tridion"),
        "tridion.meta.stackexchange.com" | "meta.tridion.stackexchange.com" => Some("tridion.meta"),
        "reverseengineering.stackexchange.com" => Some("reverseengineering"),
        "reverseengineering.meta.stackexchange.com" | "meta.reverseengineering.stackexchange.com" => Some("reverseengineering.meta"),
        "networkengineering.stackexchange.com" => Some("networkengineering"),
        "networkengineering.meta.stackexchange.com" | "meta.networkengineering.stackexchange.com" => Some("networkengineering.meta"),
        "opendata.stackexchange.com" => Some("opendata"),
        "opendata.meta.stackexchange.com" | "meta.opendata.stackexchange.com" => Some("opendata.meta"),
        "freelancing.stackexchange.com" => Some("freelancing"),
        "freelancing.meta.stackexchange.com" | "meta.freelancing.stackexchange.com" => Some("freelancing.meta"),
        "blender.stackexchange.com" => Some("blender"),
        "blender.meta.stackexchange.com" | "meta.blender.stackexchange.com" => Some("blender.meta"),
        "mathoverflow.net" | "mathoverflow.stackexchange.com" | "mathoverflow.com" => Some("mathoverflow.net"),
        "meta.mathoverflow.net" => Some("meta.mathoverflow.net"),
        "space.stackexchange.com" | "thefinalfrontier.stackexchange.com" => Some("space"),
        "space.meta.stackexchange.com" | "meta.space.stackexchange.com" => Some("space.meta"),
        "sound.stackexchange.com" | "socialsounddesign.com" | "sounddesign.stackexchange.com" => Some("sound"),
        "sound.meta.stackexchange.com" | "meta.sound.stackexchange.com" => Some("sound.meta"),
        "astronomy.stackexchange.com" => Some("astronomy"),
        "astronomy.meta.stackexchange.com" | "meta.astronomy.stackexchange.com" => Some("astronomy.meta"),
        "tor.stackexchange.com" => Some("tor"),
        "tor.meta.stackexchange.com" | "meta.tor.stackexchange.com" => Some("tor.meta"),
        "pets.stackexchange.com" => Some("pets"),
        "pets.meta.stackexchange.com" | "meta.pets.stackexchange.com" => Some("pets.meta"),
        "ham.stackexchange.com" => Some("ham"),
        "ham.meta.stackexchange.com" | "meta.ham.stackexchange.com" => Some("ham.meta"),
        "italian.stackexchange.com" => Some("italian"),
        "italian.meta.stackexchange.com" | "meta.italian.stackexchange.com" => Some("italian.meta"),
        "pt.stackoverflow.com" | "br.stackoverflow.com" | "stackoverflow.com.br" => Some("pt.stackoverflow"),
        "pt.meta.stackoverflow.com" | "meta.br.stackoverflow.com" | "meta.pt.stackoverflow.com" => Some("pt.meta.stackoverflow"),
        "aviation.stackexchange.com" => Some("aviation"),
        "aviation.meta.stackexchange.com" | "meta.aviation.stackexchange.com" => Some("aviation.meta"),
        "ebooks.stackexchange.com" => Some("ebooks"),
        "ebooks.meta.stackexchange.com" | "meta.ebooks.stackexchange.com" => Some("ebooks.meta"),
        "alcohol.stackexchange.com" | "beer.stackexchange.com" | "dranks.stackexchange.com" => Some("alcohol"),
        "alcohol.meta.stackexchange.com" | "meta.beer.stackexchange.com" | "meta.alcohol.stackexchange.com" | "beer.meta.stackexchange.com" => Some("alcohol.meta"),
        "softwarerecs.stackexchange.com" => Some("softwarerecs"),
        "softwarerecs.meta.stackexchange.com" | "meta.softwarerecs.stackexchange.com" => Some("softwarerecs.meta"),
        "arduino.stackexchange.com" => Some("arduino"),
        "arduino.meta.stackexchange.com" | "meta.arduino.stackexchange.com" => Some("arduino.meta"),
        "cs50.stackexchange.com" => Some("cs50"),
        "cs50.meta.stackexchange.com" | "meta.cs50.stackexchange.com" => Some("cs50.meta"),
        "expatriates.stackexchange.com" | "expats.stackexchange.com" => Some("expatriates"),
        "expatriates.meta.stackexchange.com" | "meta.expatriates.stackexchange.com" => Some("expatriates.meta"),
        "matheducators.stackexchange.com" => Some("matheducators"),
        "matheducators.meta.stackexchange.com" | "meta.matheducators.stackexchange.com" => Some("matheducators.meta"),
        "meta.stackoverflow.com" => Some("meta.stackoverflow"),
        "earthscience.stackexchange.com" => Some("earthscience"),
        "earthscience.meta.stackexchange.com" | "meta.earthscience.stackexchange.com" => Some("earthscience.meta"),
        "joomla.stackexchange.com" => Some("joomla"),
        "joomla.meta.stackexchange.com" | "meta.joomla.stackexchange.com" => Some("joomla.meta"),
        "datascience.stackexchange.com" => Some("datascience"),
        "datascience.meta.stackexchange.com" | "meta.datascience.stackexchange.com" => Some("datascience.meta"),
        "puzzling.stackexchange.com" => Some("puzzling"),
        "puzzling.meta.stackexchange.com" | "meta.puzzling.stackexchange.com" => Some("puzzling.meta"),
        "craftcms.stackexchange.com" => Some("craftcms"),
        "craftcms.meta.stackexchange.com" | "meta.craftcms.stackexchange.com" => Some("craftcms.meta"),
        "buddhism.stackexchange.com" => Some("buddhism"),
        "buddhism.meta.stackexchange.com" | "meta.buddhism.stackexchange.com" => Some("buddhism.meta"),
        "hinduism.stackexchange.com" => Some("hinduism"),
        "hinduism.meta.stackexchange.com" | "meta.hinduism.stackexchange.com" => Some("hinduism.meta"),
        "communitybuilding.stackexchange.com" | "moderator.stackexchange.com" | "moderators.stackexchange.com" => Some("communitybuilding"),
        "communitybuilding.meta.stackexchange.com" | "meta.moderators.stackexchange.com" | "meta.communitybuilding.stackexchange.com" => Some("communitybuilding.meta"),
        "worldbuilding.stackexchange.com" => Some("worldbuilding"),
        "worldbuilding.meta.stackexchange.com" | "meta.worldbuilding.stackexchange.com" => Some("worldbuilding.meta"),
        "ja.stackoverflow.com" | "jp.stackoverflow.com" => Some("ja.stackoverflow"),
        "ja.meta.stackoverflow.com" | "meta.ja.stackoverflow.com" => Some("ja.meta.stackoverflow"),
        "emacs.stackexchange.com" => Some("emacs"),
        "emacs.meta.stackexchange.com" | "meta.emacs.stackexchange.com" => Some("emacs.meta"),
        "hsm.stackexchange.com" => Some("hsm"),
        "hsm.meta.stackexchange.com" | "meta.hsm.stackexchange.com" => Some("hsm.meta"),
        "economics.stackexchange.com" => Some("economics"),
        "economics.meta.stackexchange.com" | "meta.economics.stackexchange.com" => Some("economics.meta"),
        "lifehacks.stackexchange.com" => Some("lifehacks"),
        "lifehacks.meta.stackexchange.com" | "meta.lifehacks.stackexchange.com" => Some("lifehacks.meta"),
        "engineering.stackexchange.com" => Some("engineering"),
        "engineering.meta.stackexchange.com" | "meta.engineering.stackexchange.com" => Some("engineering.meta"),
        "coffee.stackexchange.com" => Some("coffee"),
        "coffee.meta.stackexchange.com" | "meta.coffee.stackexchange.com" => Some("coffee.meta"),
        "vi.stackexchange.com" | "vim.stackexchange.com" => Some("vi"),
        "vi.meta.stackexchange.com" | "meta.vi.stackexchange.com" => Some("vi.meta"),
        "musicfans.stackexchange.com" => Some("musicfans"),
        "musicfans.meta.stackexchange.com" | "meta.musicfans.stackexchange.com" => Some("musicfans.meta"),
        "woodworking.stackexchange.com" => Some("woodworking"),
        "woodworking.meta.stackexchange.com" | "meta.woodworking.stackexchange.com" => Some("woodworking.meta"),
        "civicrm.stackexchange.com" => Some("civicrm"),
        "civicrm.meta.stackexchange.com" | "meta.civicrm.stackexchange.com" => Some("civicrm.meta"),
        "medicalsciences.stackexchange.com" | "health.stackexchange.com" => Some("medicalsciences"),
        "medicalsciences.meta.stackexchange.com" | "meta.health.stackexchange.com" | "health.meta.stackexchange.com" => Some("medicalsciences.meta"),
        "ru.stackoverflow.com" | "hashcode.ru" | "stackoverflow.ru" => Some("ru.stackoverflow"),
        "ru.meta.stackoverflow.com" | "meta.hashcode.ru" | "meta.ru.stackoverflow.com" => Some("ru.meta.stackoverflow"),
        "rus.stackexchange.com" | "russ.hashcode.ru" | "russ.stackexchange.com" => Some("rus"),
        "rus.meta.stackexchange.com" | "meta.rus.stackexchange.com" => Some("rus.meta"),
        "mythology.stackexchange.com" => Some("mythology"),
        "mythology.meta.stackexchange.com" | "meta.mythology.stackexchange.com" => Some("mythology.meta"),
        "law.stackexchange.com" => Some("law"),
        "law.meta.stackexchange.com" | "meta.law.stackexchange.com" => Some("law.meta"),
        "opensource.stackexchange.com" => Some("opensource"),
        "opensource.meta.stackexchange.com" | "meta.opensource.stackexchange.com" => Some("opensource.meta"),
        "elementaryos.stackexchange.com" => Some("elementaryos"),
        "elementaryos.meta.stackexchange.com" | "meta.elementaryos.stackexchange.com" => Some("elementaryos.meta"),
        "portuguese.stackexchange.com" => Some("portuguese"),
        "portuguese.meta.stackexchange.com" | "meta.portuguese.stackexchange.com" => Some("portuguese.meta"),
        "computergraphics.stackexchange.com" => Some("computergraphics"),
        "computergraphics.meta.stackexchange.com" | "meta.computergraphics.stackexchange.com" => Some("computergraphics.meta"),
        "hardwarerecs.stackexchange.com" => Some("hardwarerecs"),
        "hardwarerecs.meta.stackexchange.com" | "meta.hardwarerecs.stackexchange.com" => Some("hardwarerecs.meta"),
        "es.stackoverflow.com" => Some("es.stackoverflow"),
        "es.meta.stackoverflow.com" | "meta.es.stackoverflow.com" => Some("es.meta.stackoverflow"),
        "3dprinting.stackexchange.com" | "threedprinting.stackexchange.com" => Some("3dprinting"),
        "3dprinting.meta.stackexchange.com" | "meta.3dprinting.stackexchange.com" => Some("3dprinting.meta"),
        "ethereum.stackexchange.com" => Some("ethereum"),
        "ethereum.meta.stackexchange.com" | "meta.ethereum.stackexchange.com" => Some("ethereum.meta"),
        "latin.stackexchange.com" => Some("latin"),
        "latin.meta.stackexchange.com" | "meta.latin.stackexchange.com" => Some("latin.meta"),
        "languagelearning.stackexchange.com" => Some("languagelearning"),
        "languagelearning.meta.stackexchange.com" | "meta.languagelearning.stackexchange.com" => Some("languagelearning.meta"),
        "retrocomputing.stackexchange.com" => Some("retrocomputing"),
        "retrocomputing.meta.stackexchange.com" | "meta.retrocomputing.stackexchange.com" => Some("retrocomputing.meta"),
        "crafts.stackexchange.com" => Some("crafts"),
        "crafts.meta.stackexchange.com" | "meta.crafts.stackexchange.com" => Some("crafts.meta"),
        "korean.stackexchange.com" => Some("korean"),
        "korean.meta.stackexchange.com" | "meta.korean.stackexchange.com" => Some("korean.meta"),
        "monero.stackexchange.com" => Some("monero"),
        "monero.meta.stackexchange.com" | "meta.monero.stackexchange.com" => Some("monero.meta"),
        "ai.stackexchange.com" => Some("ai"),
        "ai.meta.stackexchange.com" | "meta.ai.stackexchange.com" => Some("ai.meta"),
        "esperanto.stackexchange.com" => Some("esperanto"),
        "esperanto.meta.stackexchange.com" | "meta.esperanto.stackexchange.com" => Some("esperanto.meta"),
        "sitecore.stackexchange.com" => Some("sitecore"),
        "sitecore.meta.stackexchange.com" | "meta.sitecore.stackexchange.com" => Some("sitecore.meta"),
        "iot.stackexchange.com" => Some("iot"),
        "iot.meta.stackexchange.com" | "meta.iot.stackexchange.com" => Some("iot.meta"),
        "literature.stackexchange.com" => Some("literature"),
        "literature.meta.stackexchange.com" | "meta.literature.stackexchange.com" => Some("literature.meta"),
        "vegetarianism.stackexchange.com" | "vegetarian.stackexchange.com" | "veg.stackexchange.com" => Some("vegetarianism"),
        "vegetarianism.meta.stackexchange.com" | "meta.vegetarian.stackexchange.com" | "meta.vegetarianism.stackexchange.com" | "veg.meta.stackexchange.com" => Some("vegetarianism.meta"),
        "ukrainian.stackexchange.com" => Some("ukrainian"),
        "ukrainian.meta.stackexchange.com" | "meta.ukrainian.stackexchange.com" => Some("ukrainian.meta"),
        "devops.stackexchange.com" => Some("devops"),
        "devops.meta.stackexchange.com" | "meta.devops.stackexchange.com" => Some("devops.meta"),
        "bioinformatics.stackexchange.com" => Some("bioinformatics"),
        "bioinformatics.meta.stackexchange.com" => Some("bioinformatics.meta"),
        "cseducators.stackexchange.com" => Some("cseducators"),
        "cseducators.meta.stackexchange.com" => Some("cseducators.meta"),
        "interpersonal.stackexchange.com" | "interpersonalskills.stackexchange.com" | "ips.stackexchange.com" => Some("interpersonal"),
        "interpersonal.meta.stackexchange.com" | "interpersonalskills.meta.stackexchange.com" | "ips.meta.stackexchange.com" => Some("interpersonal.meta"),
        "iota.stackexchange.com" => Some("iota"),
        "iota.meta.stackexchange.com" => Some("iota.meta"),
        "stellar.stackexchange.com" => Some("stellar"),
        "stellar.meta.stackexchange.com" => Some("stellar.meta"),
        "conlang.stackexchange.com" => Some("conlang"),
        "conlang.meta.stackexchange.com" => Some("conlang.meta"),
        "quantumcomputing.stackexchange.com" => Some("quantumcomputing"),
        "quantumcomputing.meta.stackexchange.com" => Some("quantumcomputing.meta"),
        "eosio.stackexchange.com" => Some("eosio"),
        "eosio.meta.stackexchange.com" => Some("eosio.meta"),
        "tezos.stackexchange.com" => Some("tezos"),
        "tezos.meta.stackexchange.com" => Some("tezos.meta"),
        "or.stackexchange.com" => Some("or"),
        "or.meta.stackexchange.com" => Some("or.meta"),
        "drones.stackexchange.com" => Some("drones"),
        "drones.meta.stackexchange.com" => Some("drones.meta"),
        "mattermodeling.stackexchange.com" | "materials.stackexchange.com" => Some("mattermodeling"),
        "mattermodeling.meta.stackexchange.com" | "materials.meta.stackexchange.com" => Some("mattermodeling.meta"),
        "cardano.stackexchange.com" => Some("cardano"),
        "cardano.meta.stackexchange.com" => Some("cardano.meta"),
        "proofassistants.stackexchange.com" => Some("proofassistants"),
        "proofassistants.meta.stackexchange.com" => Some("proofassistants.meta"),
        "substrate.stackexchange.com" | "polkadot.stackexchange.com" => Some("substrate"),
        "substrate.meta.stackexchange.com" | "polkadot.meta.stackexchange.com" => Some("substrate.meta"),
        "bioacoustics.stackexchange.com" => Some("bioacoustics"),
        "bioacoustics.meta.stackexchange.com" => Some("bioacoustics.meta"),
        "solana.stackexchange.com" => Some("solana"),
        "solana.meta.stackexchange.com" => Some("solana.meta"),
        "langdev.stackexchange.com" | "languagedesign.stackexchange.com" => Some("langdev"),
        "langdev.meta.stackexchange.com" | "languagedesign.meta.stackexchange.com" => Some("langdev.meta"),
        "genai.stackexchange.com" => Some("genai"),
        "genai.meta.stackexchange.com" => Some("genai.meta"),
        _ => None,
    }
}

pub(crate) fn process(agent: &Agent, url: &Url) -> Option<anyhow::Result<Content>> {
    let site_name = url.host_str().and_then(site_tag)?;

    Some((|| {
        let path_segments: Vec<_> = url
            .path_segments()
            .unwrap_or_else(|| "".split('/'))
            .collect();
        if path_segments.len() < 2 {
            bail!("Unknown stackoverflow URL format");
        }

        if path_segments[0] == "a" {
            let id = if path_segments[0] == "a" {
                path_segments[1]
            } else {
                path_segments[3]
            };
            let mut answers: Items<Answer> = agent
                .get(&format!(
                    "{API_BASE}answers/{id}?site={site_name}&filter={FILTER}"
                ))
                .call()?
                .into_json()?;
            let Some(answer) = answers.items.pop() else {
                bail!("Unexpected answer response: {answers:?}");
            };

            Ok(Content::Text(TextType::Post(answer.render(url))))
        } else if matches!(path_segments[0], "q" | "questions") {
            let id = path_segments[1];
            let mut questions: Items<Question> = agent
                .get(&format!(
                    "{API_BASE}questions/{id}?site={site_name}&filter={FILTER}"
                ))
                .call()?
                .into_json()?;
            let Some(question) = questions.items.pop() else {
                bail!("Unexpected question response: {questions:?}");
            };

            let question_post = Post {
                author: question.owner.display_name,
                body: html::render(&question.body, url),
                urls: vec![],
            };

            Ok(Content::Text(TextType::PostThread(
                if let Some(answer_id) = path_segments.get(3).and_then(|s| s.parse::<u64>().ok()) {
                    let answer = question
                        .answers
                        .and_then(|a| a.into_iter().find(|a| a.answer_id == answer_id))
                        .context("question {id} missing requested answer id {answer_id}")?;
                    PostThread {
                        before: vec![question_post],
                        main: answer.render(url),
                        after: vec![],
                    }
                } else {
                    PostThread {
                        before: vec![],
                        main: question_post,
                        after: question
                            .answers
                            .unwrap_or_else(Vec::new)
                            .into_iter()
                            .map(|a| a.render(url))
                            .collect(),
                    }
                },
            )))
        } else {
            bail!("Unknown stackoverflow URL format");
        }
    })())
}

#[derive(Debug, Deserialize)]
struct Items<T> {
    items: Vec<T>,
}

#[derive(Debug, Deserialize)]
struct Answer {
    #[allow(clippy::struct_field_names)]
    answer_id: u64,
    body: String,
    owner: User,
}

#[derive(Debug, Deserialize)]
struct Question {
    answers: Option<Vec<Answer>>,
    body: String,
    owner: User,
}

#[derive(Debug, Deserialize)]
struct User {
    display_name: String,
}

impl Answer {
    fn render(self, url: &Url) -> Post {
        Post {
            author: html::render(&self.owner.display_name, url),
            body: html::render(&self.body, url),
            urls: vec![],
        }
    }
}
