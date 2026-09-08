//! Pack Rime Cangjie 5 dictionaries into JSON for the browser client.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const RADICALS: &[(&str, &str)] = &[
    ("a", "日"),
    ("b", "月"),
    ("c", "金"),
    ("d", "木"),
    ("e", "水"),
    ("f", "火"),
    ("g", "土"),
    ("h", "竹"),
    ("i", "戈"),
    ("j", "十"),
    ("k", "大"),
    ("l", "中"),
    ("m", "一"),
    ("n", "弓"),
    ("o", "人"),
    ("p", "心"),
    ("q", "手"),
    ("r", "口"),
    ("s", "尸"),
    ("t", "廿"),
    ("u", "山"),
    ("v", "女"),
    ("w", "田"),
    ("x", "難"),
    ("y", "卜"),
    ("z", "重"),
];

/// Everyday characters to bias early waves toward (still need a Rime code).
const COMMON: &[&str] = &[
    "的", "一", "是", "不", "了", "人", "我", "在", "有", "他", "這", "中", "大", "來", "上",
    "國", "個", "到", "說", "們", "為", "子", "和", "你", "地", "出", "道", "也", "時", "年",
    "得", "就", "那", "要", "下", "以", "生", "會", "自", "著", "去", "之", "過", "家", "學",
    "對", "可", "她", "裡", "後", "小", "麼", "心", "多", "天", "而", "能", "好", "都", "然",
    "沒", "日", "於", "起", "還", "發", "成", "事", "只", "作", "當", "想", "看", "文", "無",
    "開", "手", "十", "用", "主", "行", "方", "又", "如", "前", "所", "本", "見", "經", "頭",
    "面", "公", "同", "三", "已", "動", "兩", "長", "知", "民", "樣", "現", "身", "理", "實",
    "法", "走", "作", "部", "分", "種", "月", "定", "二", "回", "力", "水", "問", "意", "建",
    "物", "口", "門", "女", "山", "木", "火", "土", "金", "目", "田", "竹", "雨", "風", "馬",
    "鳥", "魚", "車", "食", "言", "糸", "草", "花", "春", "秋", "冬", "夏", "明", "星", "光",
    "電", "氣", "聲", "色", "白", "青", "紅", "黑", "黃", "東", "西", "南", "北", "高", "正",
    "新", "老", "少", "美", "樂", "愛", "友", "父", "母", "兄", "弟", "姐", "妹", "子", "孫",
];

fn main() {
    if let Err(e) = run() {
        eprintln!("cjpack: {e}");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let root = find_root();
    let rime = root.join("rime/cangjie5.base.dict.yaml");
    let out_json = root.join("web/cangjie.json");

    fs::create_dir_all(out_json.parent().unwrap_or(Path::new(".")))?;

    let yaml = fs::read_to_string(&rime)?;
    let table = parse_rime(&yaml);
    write_json(&out_json, &table)?;

    println!(
        "packed {} characters -> {}",
        table.len(),
        out_json.display()
    );
    Ok(())
}

fn find_root() -> PathBuf {
    let mut dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    for _ in 0..6 {
        if dir.join("rime/cangjie5.base.dict.yaml").is_file() {
            return dir;
        }
        if !dir.pop() {
            break;
        }
    }
    PathBuf::from(".")
}

fn parse_rime(yaml: &str) -> BTreeMap<String, String> {
    let mut in_body = false;
    let mut best: BTreeMap<String, String> = BTreeMap::new();

    for line in yaml.lines() {
        let line = line.trim_end();
        if !in_body {
            if line == "..." {
                in_body = true;
            }
            continue;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split('\t');
        let han = match parts.next() {
            Some(s) => s.trim(),
            None => continue,
        };
        let code = match parts.next() {
            Some(s) => s.trim().to_ascii_lowercase(),
            None => continue,
        };
        if !is_hanzi(han) {
            continue;
        }
        if !is_cangjie_code(&code) {
            continue;
        }
        match best.get(han) {
            None => {
                best.insert(han.to_string(), code);
            }
            Some(old) => {
                if code.len() < old.len() || (code.len() == old.len() && code < *old) {
                    best.insert(han.to_string(), code);
                }
            }
        }
    }
    best
}

fn is_hanzi(s: &str) -> bool {
    let mut chars = s.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => {
            let u = c as u32;
            (0x4E00..=0x9FFF).contains(&u) || (0x3400..=0x4DBF).contains(&u)
        }
        _ => false,
    }
}

fn is_cangjie_code(code: &str) -> bool {
    let n = code.len();
    if !(1..=5).contains(&n) {
        return false;
    }
    code.bytes().all(|b| matches!(b, b'a'..=b'z'))
}

fn write_json(path: &Path, table: &BTreeMap<String, String>) -> io::Result<()> {
    let common: std::collections::HashSet<&str> = COMMON.iter().copied().collect();
    let mut buf = String::with_capacity(table.len() * 48);
    buf.push_str("{\n  \"source\": \"Rime cangjie5.base (五倉世紀)\",\n");
    buf.push_str("  \"radicals\": {\n");
    for (i, (k, v)) in RADICALS.iter().enumerate() {
        buf.push_str(&format!("    \"{k}\": \"{v}\""));
        buf.push_str(if i + 1 == RADICALS.len() {
            "\n"
        } else {
            ",\n"
        });
    }
    buf.push_str("  },\n  \"chars\": [\n");

    let mut first = true;
    for (han, code) in table {
        if !first {
            buf.push_str(",\n");
        }
        first = false;
        let freq = if common.contains(han.as_str()) { 1 } else { 0 };
        buf.push_str(&format!(
            "    {{\"h\":\"{han}\",\"c\":\"{code}\",\"n\":{},\"f\":{freq}}}",
            code.len()
        ));
    }
    buf.push_str("\n  ]\n}\n");
    fs::write(path, buf)
}
