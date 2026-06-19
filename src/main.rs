use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Semaphore;
use genpdf::{elements, style, Document, Element, SimplePageDecorator};
use tauri::Emitter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ItemType {
    Module,
    Struct,
    Enum,
    Trait,
    Function,
    Macro,
    DeriveMacro,
    AttributeMacro,
    TypeDefinition,
    Constant,
    Union,
    Unknown,
}

impl ItemType {
    fn from_filename(filename: &str) -> Option<Self> {
        if filename.starts_with("struct.") {
            Some(ItemType::Struct)
        } else if filename.starts_with("enum.") {
            Some(ItemType::Enum)
        } else if filename.starts_with("trait.") {
            Some(ItemType::Trait)
        } else if filename.starts_with("fn.") {
            Some(ItemType::Function)
        } else if filename.starts_with("macro.") {
            Some(ItemType::Macro)
        } else if filename.starts_with("derive.") {
            Some(ItemType::DeriveMacro)
        } else if filename.starts_with("attr.") {
            Some(ItemType::AttributeMacro)
        } else if filename.starts_with("type.") {
            Some(ItemType::TypeDefinition)
        } else if filename.starts_with("constant.") {
            Some(ItemType::Constant)
        } else if filename.starts_with("union.") {
            Some(ItemType::Union)
        } else if filename.starts_with("module.") {
            Some(ItemType::Module)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
enum DocElement {
    Heading(String, String),
    Paragraph(String),
    CodeBlock(String),
    List(Vec<String>),
}

struct ScrapingResult {
    name: String,
    decl: String,
    docs: Vec<DocElement>,
}

#[derive(Clone)]
struct ItemDoc {
    name: String,
    module_path: String,
    url: String,
    decl: String,
    docs: Vec<DocElement>,
}

#[derive(Clone)]
struct ModuleDoc {
    name: String,
    url: String,
    overview: Vec<DocElement>,
}

struct CrateDocumentation {
    name: String,
    overview: Vec<DocElement>,
    modules: Vec<ModuleDoc>,
    traits: Vec<ItemDoc>,
    structs: Vec<ItemDoc>,
    enums: Vec<ItemDoc>,
    functions: Vec<ItemDoc>,
    macros: Vec<ItemDoc>,
    type_definitions: Vec<ItemDoc>,
    constants: Vec<ItemDoc>,
    unions: Vec<ItemDoc>,
}

#[derive(Deserialize)]
struct CratesIoResponse {
    crates: Vec<CrateInfo>,
}

#[derive(Deserialize)]
struct CrateInfo {
    name: String,
    description: Option<String>,
    documentation: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
struct CrateResult {
    name: String,
    desc: String,
    href: String,
}

#[derive(Clone, Serialize)]
struct ProgressPayload {
    progress: f32,
    status: String,
}

#[tauri::command]
async fn search_crates(query: String) -> Result<Vec<CrateResult>, String> {
    println!("Processando busca para: {}", query);
    perform_search(&query).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn generate_pdf(
    window: tauri::Window,
    name: String,
    href: String,
    translate: bool,
) -> Result<String, String> {
    println!("Iniciando manual PDF para '{}'...", name);
    perform_generate(&name, &href, translate, window).await.map_err(|e| e.to_string())
}

fn main() {
    println!("Iniciando Interface Gráfica (Tauri)...");
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![search_crates, generate_pdf])
        .setup(|app| {
            use tauri::Manager;
            if let Some(window) = app.get_webview_window("main") {
                let icon_bytes = include_bytes!("../icons/icon.png");
                if let Ok(icon) = tauri::image::Image::from_bytes(icon_bytes) {
                    let _ = window.set_icon(icon);
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}


async fn perform_search(query: &str) -> Result<Vec<CrateResult>, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::builder()
        .user_agent("crates_docs_pdf/0.1.0 (contact@example.com)")
        .build()?;
        
    let url = format!("https://crates.io/api/v1/crates?q={}&per_page=5", query);
    let res = client.get(&url).send().await?;
    if !res.status().is_success() {
        return Err(format!("Erro ao acessar API do crates.io: {}", res.status()).into());
    }
    
    let body = res.text().await?;
    let response: CratesIoResponse = serde_json::from_str(&body)?;
    let mut results = Vec::new();
    for c in response.crates {
        let documentation = c.documentation.unwrap_or_else(|| format!("https://docs.rs/{}", c.name));
        results.push(CrateResult {
            name: c.name,
            desc: c.description.unwrap_or_else(|| "Sem descrição disponível.".to_string()),
            href: documentation,
        });
    }
    Ok(results)
}

fn clean_heading_text(text: &str) -> String {
    let mut cleaned = text.replace('§', "");
    cleaned = cleaned.replace('ⓘ', "");
    cleaned.trim().to_string()
}

fn parse_docblock(element: scraper::ElementRef) -> Vec<DocElement> {
    let mut elements = Vec::new();
    for child in element.children() {
        if let Some(el) = scraper::ElementRef::wrap(child) {
            let tag = el.value().name();
            match tag {
                "p" => {
                    let text = el.text().collect::<Vec<_>>().join(" ").trim().to_string();
                    if !text.is_empty() {
                        elements.push(DocElement::Paragraph(text));
                    }
                }
                "pre" => {
                    let text = el.text().collect::<Vec<_>>().join("").trim().to_string();
                    if !text.is_empty() {
                        elements.push(DocElement::CodeBlock(text));
                    }
                }
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    let text = el.text().collect::<Vec<_>>().join(" ").trim().to_string();
                    let cleaned = clean_heading_text(&text);
                    if !cleaned.is_empty() {
                        elements.push(DocElement::Heading(tag.to_string(), cleaned));
                    }
                }
                "ul" | "ol" => {
                    let li_selector = scraper::Selector::parse("li").unwrap();
                    let mut items = Vec::new();
                    for li in el.select(&li_selector) {
                        let li_text = li.text().collect::<Vec<_>>().join(" ").trim().to_string();
                        if !li_text.is_empty() {
                            items.push(li_text);
                        }
                    }
                    if !items.is_empty() {
                        elements.push(DocElement::List(items));
                    }
                }
                _ => {
                    elements.extend(parse_docblock(el));
                }
            }
        }
    }
    elements
}

fn parse_item_content(html: &str) -> Option<ScrapingResult> {
    let document = scraper::Html::parse_document(html);
    
    let main_heading_selector = scraper::Selector::parse("h1").ok()?;
    let main_heading = document.select(&main_heading_selector).next()?;
    let name = main_heading.text().collect::<Vec<_>>().join(" ").trim().to_string();
    let name = clean_heading_text(&name);
    
    let decl_selector = scraper::Selector::parse(".item-decl, pre.rust").ok()?;
    let decl = if let Some(decl_el) = document.select(&decl_selector).next() {
        decl_el.text().collect::<Vec<_>>().join("").trim().to_string()
    } else {
        String::new()
    };
    
    let main_content_selector = scraper::Selector::parse("#main-content, main").ok()?;
    let main_content = document.select(&main_content_selector).next()?;
    
    let mut docs = Vec::new();
    
    for child in main_content.children() {
        if let Some(element) = scraper::ElementRef::wrap(child) {
            let id = element.value().attr("id").unwrap_or("");
            
            if id == "trait-implementations" || id == "synthetic-implementations" || id == "blanket-implementations" {
                break;
            }
            
            let tag = element.value().name();
            match tag {
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    let text = element.text().collect::<Vec<_>>().join(" ").trim().to_string();
                    let cleaned = clean_heading_text(&text);
                    if !cleaned.is_empty() {
                        docs.push(DocElement::Heading(tag.to_string(), cleaned));
                    }
                }
                "p" => {
                    let text = element.text().collect::<Vec<_>>().join(" ").trim().to_string();
                    if !text.is_empty() {
                        docs.push(DocElement::Paragraph(text));
                    }
                }
                "pre" => {
                    if element.value().classes().any(|c| c == "item-decl") {
                        continue;
                    }
                    let text = element.text().collect::<Vec<_>>().join("").trim().to_string();
                    if !text.is_empty() {
                        docs.push(DocElement::CodeBlock(text));
                    }
                }
                _ => {
                    if element.value().classes().any(|c| c == "docblock") {
                        docs.extend(parse_docblock(element));
                    } else {
                        let docblock_selector = scraper::Selector::parse(".docblock").unwrap();
                        for db in element.select(&docblock_selector) {
                            docs.extend(parse_docblock(db));
                        }
                    }
                }
            }
        }
    }
    
    Some(ScrapingResult { name, decl, docs })
}

fn parse_crate_overview(html: &str) -> Vec<DocElement> {
    let document = scraper::Html::parse_document(html);
    let mut docs = Vec::new();
    
    let main_content_selector = scraper::Selector::parse("#main-content, main").unwrap();
    let Some(main_content) = document.select(&main_content_selector).next() else {
        return docs;
    };
    
    for child in main_content.children() {
        if let Some(element) = scraper::ElementRef::wrap(child) {
            let id = element.value().attr("id").unwrap_or("");
            
            if id == "modules" || id == "structs" || id == "enums" || id == "traits" || id == "macros" || id == "functions" || id == "derives" || id == "types" || id == "constants" || id == "reexports" {
                break;
            }
            
            let tag = element.value().name();
            match tag {
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    let text = element.text().collect::<Vec<_>>().join(" ").trim().to_string();
                    let cleaned = clean_heading_text(&text);
                    if !cleaned.is_empty() {
                        docs.push(DocElement::Heading(tag.to_string(), cleaned));
                    }
                }
                "p" => {
                    let text = element.text().collect::<Vec<_>>().join(" ").trim().to_string();
                    if !text.is_empty() {
                        docs.push(DocElement::Paragraph(text));
                    }
                }
                "pre" => {
                    let text = element.text().collect::<Vec<_>>().join("").trim().to_string();
                    if !text.is_empty() {
                        docs.push(DocElement::CodeBlock(text));
                    }
                }
                _ => {
                    if element.value().classes().any(|c| c == "docblock") {
                        docs.extend(parse_docblock(element));
                    } else {
                        let docblock_selector = scraper::Selector::parse(".docblock").unwrap();
                        for db in element.select(&docblock_selector) {
                            docs.extend(parse_docblock(db));
                        }
                    }
                }
            }
        }
    }
    
    docs
}

fn get_crate_base_url(url: &str) -> String {
    let mut u = url.to_string();
    if u.ends_with("index.html") {
        u = u.replace("index.html", "");
    }
    if !u.ends_with('/') {
        u.push('/');
    }
    u
}


// -------------------------------------------------------------
// Translation Pipeline Functions
// -------------------------------------------------------------

async fn translate_single_paragraph(
    client: &reqwest::Client,
    text: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let encoded_text: String = url::form_urlencoded::byte_serialize(text.as_bytes()).collect();
    let url = format!(
        "https://translate.googleapis.com/translate_a/single?client=gtx&sl=en&tl=pt&dt=t&q={}",
        encoded_text
    );
    let res = client.get(&url).send().await?;
        
    if !res.status().is_success() {
        return Err(format!("Erro na API de tradução: {}", res.status()).into());
    }
    
    let json: serde_json::Value = res.json().await?;
    let mut translated = String::new();
    if let Some(outer_arr) = json.as_array() {
        if let Some(inner_arr) = outer_arr.get(0).and_then(|v| v.as_array()) {
            for item in inner_arr {
                if let Some(sentence_arr) = item.as_array() {
                    if let Some(sentence) = sentence_arr.get(0).and_then(|v| v.as_str()) {
                        translated.push_str(sentence);
                    }
                }
            }
        }
    }
    
    if translated.is_empty() {
        Ok(text.to_string())
    } else {
        let cleaned = translated.replace('☐', "O");
        Ok(cleaned)
    }
}

fn protect_text(
    text: &str,
    protected_terms: &std::collections::HashSet<String>,
) -> (String, Vec<String>) {
    let mut tokens = Vec::new();
    let mut result = String::new();
    let mut current_word = String::new();
    
    for c in text.chars() {
        if c.is_alphanumeric() || c == '_' {
            current_word.push(c);
        } else {
            if !current_word.is_empty() {
                if protected_terms.contains(&current_word) {
                    let placeholder = format!("Zpt{}Z", tokens.len());
                    tokens.push(current_word.clone());
                    result.push_str(&placeholder);
                } else {
                    result.push_str(&current_word);
                }
                current_word.clear();
            }
            result.push(c);
        }
    }
    
    if !current_word.is_empty() {
        if protected_terms.contains(&current_word) {
            let placeholder = format!("Zpt{}Z", tokens.len());
            tokens.push(current_word.clone());
            result.push_str(&placeholder);
        } else {
            result.push_str(&current_word);
        }
    }
    
    (result, tokens)
}

fn restore_text(translated: &str, tokens: &[String]) -> String {
    let mut result = translated.to_string();
    
    // Clean up translator specific bugs
    result = result.replace('☐', "O");
    result = result.replace("☐ ", "O ");
    
    for (i, token) in tokens.iter().enumerate() {
        let placeholder = format!("Zpt{}Z", i);
        
        // Handle common spacing modifications introduced by translation services
        let variations = [
            format!("Zpt {}Z", i),
            format!("Zpt{} Z", i),
            format!("Zpt {} Z", i),
            format!("zpt {}z", i),
            format!("zpt{} z", i),
            format!("zpt {} z", i),
            format!("ZPT {}Z", i),
            format!("ZPT{} Z", i),
            format!("ZPT {} Z", i),
        ];
        
        for variation in &variations {
            result = result.replace(variation, token);
        }
        
        result = result.replace(&placeholder, token);
        result = result.replace(&placeholder.to_lowercase(), token);
        result = result.replace(&placeholder.to_uppercase(), token);
    }
    result
}

async fn translate_paragraph_with_protection(
    client: &reqwest::Client,
    text: &str,
    protected_terms: &std::collections::HashSet<String>,
) -> String {
    if text.trim().is_empty() {
        return text.to_string();
    }
    let (encoded, tokens) = protect_text(text, protected_terms);
    match translate_single_paragraph(client, &encoded).await {
        Ok(translated) => restore_text(&translated, &tokens),
        Err(e) => {
            eprintln!("Erro ao traduzir parágrafo, mantendo original: {}", e);
            text.to_string()
        }
    }
}

async fn translate_doc_elements(
    client: &reqwest::Client,
    elements: &mut [DocElement],
    protected_terms: &std::collections::HashSet<String>,
) {
    for el in elements {
        match el {
            DocElement::Paragraph(text) => {
                *text = translate_paragraph_with_protection(client, text, protected_terms).await;
            }
            DocElement::Heading(_, text) => {
                *text = translate_paragraph_with_protection(client, text, protected_terms).await;
            }
            DocElement::List(items) => {
                for item in items {
                    *item = translate_paragraph_with_protection(client, item, protected_terms).await;
                }
            }
            DocElement::CodeBlock(_) => {} // KEEP CODE BLOCKS UNTRANSLATED
        }
    }
}

async fn translate_crate_documentation(
    data: &mut CrateDocumentation,
    protected_terms: &std::collections::HashSet<String>,
    window: &tauri::Window,
) {
    let client = match reqwest::Client::builder()
        .user_agent("crates_docs_pdf/0.1.0 (contact@example.com)")
        .build() {
            Ok(c) => c,
            Err(_) => return,
        };
        
    let _ = window.emit("progress", ProgressPayload {
        progress: 0.70,
        status: "Traduzindo visão geral da crate...".to_string(),
    });
    translate_doc_elements(&client, &mut data.overview, protected_terms).await;
    
    let total_modules = data.modules.len();
    for (idx, m) in data.modules.iter_mut().enumerate() {
        let progress = 0.70 + (idx as f32 / total_modules.max(1) as f32) * 0.05; // 70% to 75%
        let _ = window.emit("progress", ProgressPayload {
            progress,
            status: format!("Traduzindo submódulo {} de {} ({})", idx + 1, total_modules, m.name),
        });
        translate_doc_elements(&client, &mut m.overview, protected_terms).await;
    }
    
    let mut all_items = Vec::new();
    for t in &mut data.traits { all_items.push((format!("trait {}", t.name), &mut t.docs)); }
    for s in &mut data.structs { all_items.push((format!("struct {}", s.name), &mut s.docs)); }
    for e in &mut data.enums { all_items.push((format!("enum {}", e.name), &mut e.docs)); }
    for f in &mut data.functions { all_items.push((format!("fn {}", f.name), &mut f.docs)); }
    for m in &mut data.macros { all_items.push((format!("macro {}", m.name), &mut m.docs)); }
    for td in &mut data.type_definitions { all_items.push((format!("type {}", td.name), &mut td.docs)); }
    for c in &mut data.constants { all_items.push((format!("const {}", c.name), &mut c.docs)); }
    for u in &mut data.unions { all_items.push((format!("union {}", u.name), &mut u.docs)); }
    
    let total_items = all_items.len();
    for (idx, (desc, item_docs)) in all_items.into_iter().enumerate() {
        let progress = 0.75 + (idx as f32 / total_items.max(1) as f32) * 0.15; // 75% to 90%
        let _ = window.emit("progress", ProgressPayload {
            progress,
            status: format!("Traduzindo item {} de {} ({})", idx + 1, total_items, desc),
        });
        translate_doc_elements(&client, item_docs, protected_terms).await;
        // Small delay to avoid Google Translate rate limits
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

// -------------------------------------------------------------
// Document Generation pipeline
// -------------------------------------------------------------

async fn perform_generate(
    name: &str,
    href: &str,
    translate: bool,
    window: tauri::Window,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::builder()
        .user_agent("crates_docs_pdf/0.1.0 (contact@example.com)")
        .build()?;

    let target_url = if href.is_empty() {
        format!("https://docs.rs/{}", name)
    } else {
        href.to_string()
    };

    let update_progress = |progress: f32, msg: &str| {
        let _ = window.emit("progress", ProgressPayload {
            progress,
            status: msg.to_string(),
        });
    };

    update_progress(0.02, &format!("Resolvendo URL da crate '{}'...", name));
    let res = client.get(&target_url).send().await?;
    let final_url = res.url().to_string();
    let base_url = get_crate_base_url(&final_url);
    let all_url = format!("{}all.html", base_url);

    update_progress(0.05, "Buscando índice de todos os itens (all.html)...");
    let all_html = client.get(&all_url).send().await?.text().await?;
    
    let parsed_items: Vec<(ItemType, String, String, String)> = {
        let document = scraper::Html::parse_document(&all_html);
        let a_selector = scraper::Selector::parse("a").unwrap();
        let mut items = Vec::new();
        
        for a in document.select(&a_selector) {
            let Some(item_href) = a.value().attr("href") else { continue; };
            let clean_href = item_href.split('#').next().unwrap().split('?').next().unwrap();
            let Some(filename) = clean_href.split('/').last() else { continue; };
            
            if let Some(item_type) = ItemType::from_filename(filename) {
                let text = a.text().collect::<Vec<_>>().join(" ").trim().to_string();
                if text.is_empty() {
                    continue;
                }
                
                let absolute_url = match reqwest::Url::parse(&base_url) {
                    Ok(parsed_base) => match parsed_base.join(clean_href) {
                        Ok(u) => u.to_string(),
                        Err(_) => format!("{}{}", base_url, clean_href),
                    },
                    Err(_) => format!("{}{}", base_url, clean_href),
                };
                
                if !items.iter().any(|(_, name, _, _)| name == &text) {
                    items.push((item_type, text, absolute_url, clean_href.to_string()));
                }
            }
        }
        items
    };

    update_progress(0.08, &format!("Encontrados {} itens. Mapeando submódulos...", parsed_items.len()));

    let mut modules = std::collections::HashSet::new();
    for (_, text, _, _) in &parsed_items {
        if text.contains("::") {
            let parts: Vec<&str> = text.split("::").collect();
            let mut current = String::new();
            for i in 0..(parts.len() - 1) {
                if !current.is_empty() {
                    current.push_str("::");
                }
                current.push_str(parts[i]);
                modules.insert(current.clone());
            }
        }
    }
    
    let mut sorted_modules: Vec<String> = modules.into_iter().collect();
    sorted_modules.sort();

    let sem = Arc::new(Semaphore::new(10));
    
    // 1. Crawl Modules concurrently
    let total_modules = sorted_modules.len();
    update_progress(0.10, &format!("Baixando documentação de {} submódulos...", total_modules));
    
    let completed_modules = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut module_tasks = Vec::new();
    for mod_name in sorted_modules {
        let relative_path = format!("{}/index.html", mod_name.replace("::", "/"));
        let absolute_mod_url = match reqwest::Url::parse(&base_url) {
            Ok(parsed_base) => match parsed_base.join(&relative_path) {
                Ok(u) => u.to_string(),
                Err(_) => format!("{}{}", base_url, relative_path),
            },
            Err(_) => format!("{}{}", base_url, relative_path),
        };
        
        let client = client.clone();
        let sem = sem.clone();
        let window = window.clone();
        let completed_modules = completed_modules.clone();
        
        module_tasks.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            
            let count = completed_modules.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            let progress = 0.10 + (count as f32 / total_modules.max(1) as f32) * 0.15; // 10% to 25%
            let _ = window.emit("progress", ProgressPayload {
                progress,
                status: format!("Baixando submódulo {}/{} ({})", count, total_modules, mod_name),
            });

            let mut html = String::new();
            for attempt in 0..3 {
                if let Ok(res) = client.get(&absolute_mod_url).send().await {
                    if res.status().is_success() {
                        if let Ok(text) = res.text().await {
                            html = text;
                            break;
                        }
                    }
                }
                tokio::time::sleep(std::time::Duration::from_millis(200 * (attempt + 1))).await;
            }
            
            if html.is_empty() {
                None
            } else {
                let overview = parse_crate_overview(&html);
                Some(ModuleDoc {
                    name: mod_name,
                    url: absolute_mod_url,
                    overview,
                })
            }
        }));
    }

    let mut module_docs = Vec::new();
    for handle in module_tasks {
        if let Ok(Some(m)) = handle.await {
            module_docs.push(m);
        }
    }
    module_docs.sort_by(|a, b| a.name.cmp(&b.name));

    // 2. Crawl Items concurrently
    let total_items = parsed_items.len();
    update_progress(0.25, &format!("Baixando documentação de {} itens...", total_items));
    
    let completed_items = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut item_tasks = Vec::new();
    for (item_type, full_name, absolute_url, _) in parsed_items.clone() {
        let client = client.clone();
        let sem = sem.clone();
        let window = window.clone();
        let completed_items = completed_items.clone();
        
        item_tasks.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            
            let count = completed_items.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            let progress = 0.25 + (count as f32 / total_items.max(1) as f32) * 0.45; // 25% to 70%
            let _ = window.emit("progress", ProgressPayload {
                progress,
                status: format!("Baixando item {}/{} ({})", count, total_items, full_name),
            });

            let mut html = String::new();
            for attempt in 0..3 {
                if let Ok(res) = client.get(&absolute_url).send().await {
                    if res.status().is_success() {
                        if let Ok(text) = res.text().await {
                            html = text;
                            break;
                        }
                    }
                }
                tokio::time::sleep(std::time::Duration::from_millis(200 * (attempt + 1))).await;
            }
            
            if html.is_empty() {
                return None;
            }
            
            if let Some(scraped) = parse_item_content(&html) {
                let (module_path, local_name) = if full_name.contains("::") {
                    let parts: Vec<&str> = full_name.split("::").collect();
                    let local = parts.last().unwrap().to_string();
                    let path = parts[0..(parts.len() - 1)].join("::");
                    (path, local)
                } else {
                    (String::new(), full_name.clone())
                };

                Some((
                    item_type,
                    ItemDoc {
                        name: local_name,
                        module_path,
                        url: absolute_url,
                        decl: scraped.decl,
                        docs: scraped.docs,
                    }
                ))
            } else {
                None
            }
        }));
    }

    let mut traits = Vec::new();
    let mut structs = Vec::new();
    let mut enums = Vec::new();
    let mut functions = Vec::new();
    let mut macros = Vec::new();
    let mut type_definitions = Vec::new();
    let mut constants = Vec::new();
    let mut unions = Vec::new();

    for handle in item_tasks {
        if let Ok(Some((item_type, item_doc))) = handle.await {
            match item_type {
                ItemType::Trait => traits.push(item_doc),
                ItemType::Struct => structs.push(item_doc),
                ItemType::Enum => enums.push(item_doc),
                ItemType::Function => functions.push(item_doc),
                ItemType::Macro | ItemType::DeriveMacro | ItemType::AttributeMacro => macros.push(item_doc),
                ItemType::TypeDefinition => type_definitions.push(item_doc),
                ItemType::Constant => constants.push(item_doc),
                ItemType::Union => unions.push(item_doc),
                _ => {}
            }
        }
    }

    traits.sort_by(|a, b| a.name.cmp(&b.name));
    structs.sort_by(|a, b| a.name.cmp(&b.name));
    enums.sort_by(|a, b| a.name.cmp(&b.name));
    functions.sort_by(|a, b| a.name.cmp(&b.name));
    macros.sort_by(|a, b| a.name.cmp(&b.name));
    type_definitions.sort_by(|a, b| a.name.cmp(&b.name));
    constants.sort_by(|a, b| a.name.cmp(&b.name));
    unions.sort_by(|a, b| a.name.cmp(&b.name));

    // Get Crate main overview
    let crate_overview = if let Ok(res) = client.get(&final_url).send().await {
        if let Ok(text) = res.text().await {
            parse_crate_overview(&text)
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    let mut doc_data = CrateDocumentation {
        name: name.to_string(),
        overview: crate_overview,
        modules: module_docs,
        traits,
        structs,
        enums,
        functions,
        macros,
        type_definitions,
        constants,
        unions,
    };

    // 3. Optional translation step
    if translate {
        update_progress(0.70, "Construindo glossário de termos protegidos...");
        
        let mut protected_terms = std::collections::HashSet::new();
        let rust_keywords = [
            "struct", "enum", "trait", "fn", "impl", "pub", "mut", "let", "match", 
            "use", "mod", "crate", "self", "Self", "const", "static", "where", 
            "type", "unsafe", "async", "await", "dyn", "ref", "move",
            "String", "Option", "Result", "Vec", "HashMap", "Box", "Rc", "Arc", 
            "Cell", "RefCell", "u8", "u16", "u32", "u64", "u128", "i8", "i16", 
            "i32", "i64", "i128", "usize", "isize", "f32", "f64", "str", "bool", "char"
        ];
        for kw in &rust_keywords {
            protected_terms.insert(kw.to_string());
        }
        
        protected_terms.insert(name.to_string());
        
        for (_, text, _, _) in &parsed_items {
            protected_terms.insert(text.clone());
            if text.contains("::") {
                for part in text.split("::") {
                    if !part.is_empty() {
                        protected_terms.insert(part.to_string());
                    }
                }
            }
        }
        
        translate_crate_documentation(&mut doc_data, &protected_terms, &window).await;
    } else {
        update_progress(0.90, "Pulando tradução...");
    }

    update_progress(0.92, "Renderizando e salvando PDF...");
    let filename = format!("{}_docs.pdf", name);
    generate_pdf_file_hierarchical(&filename, doc_data)?;

    update_progress(1.00, "Concluído!");
    Ok(filename)
}
fn render_doc_elements(
    doc: &mut Document,
    elements_list: &[DocElement],
    mono_font_ref: genpdf::fonts::FontFamily<genpdf::fonts::Font>,
) {
    let h1_style = style::Style::new().bold().with_font_size(14);
    let h2_style = style::Style::new().bold().with_font_size(12);
    let h3_style = style::Style::new().bold().with_font_size(10);
    let body_style = style::Style::new().with_font_size(10);
    let mono_style = style::Style::new().with_font_family(mono_font_ref).with_font_size(8);

    for el in elements_list {
        match el {
            DocElement::Heading(level, text) => {
                let p = elements::Paragraph::new(text);
                let styled_p = match level.as_str() {
                    "h1" => p.styled(h1_style),
                    "h2" => p.styled(h2_style),
                    _ => p.styled(h3_style),
                };
                doc.push(styled_p);
                doc.push(elements::Break::new(0.4));
            }
            DocElement::Paragraph(text) => {
                let clean = text.replace('\n', " ").trim().to_string();
                if !clean.is_empty() {
                    doc.push(elements::Paragraph::new(clean).styled(body_style));
                    doc.push(elements::Break::new(0.5));
                }
            }
            DocElement::CodeBlock(text) => {
                let mut content = elements::LinearLayout::vertical();
                for line in text.lines() {
                    content.push(elements::Paragraph::new(line).styled(mono_style));
                }
                let padded = elements::PaddedElement::new(content, genpdf::Margins::trbl(3, 5, 3, 5));
                doc.push(elements::FramedElement::new(padded));
                doc.push(elements::Break::new(0.5));
            }
            DocElement::List(items) => {
                let mut list_layout = elements::LinearLayout::vertical();
                for item in items {
                    let clean = item.replace('\n', " ").trim().to_string();
                    if !clean.is_empty() {
                        list_layout.push(elements::Paragraph::new(format!("•  {}", clean)).styled(body_style));
                    }
                }
                let indented = elements::PaddedElement::new(list_layout, genpdf::Margins::trbl(0, 0, 0, 8));
                doc.push(indented);
                doc.push(elements::Break::new(0.5));
            }
        }
    }
}

fn generate_pdf_file_hierarchical(
    filename: &str,
    data: CrateDocumentation,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let font_dir = "/usr/share/fonts/truetype/liberation";
    let font_family = genpdf::fonts::from_files(font_dir, "LiberationSans", None)?;
    let mut doc = Document::new(font_family);

    let mono_font = genpdf::fonts::from_files(font_dir, "LiberationMono", None)?;
    let mono_font_ref = doc.add_font_family(mono_font);

    let mut decorator = SimplePageDecorator::new();
    decorator.set_margins(18);
    doc.set_page_decorator(decorator);

    doc.set_title(format!("Crate Documentation: {}", data.name));

    // Cover Page
    doc.push(elements::Break::new(6.0));
    doc.push(
        elements::Paragraph::new(format!("Crate Documentation: {}", data.name))
            .styled(style::Style::new().bold().with_font_size(28)),
    );
    doc.push(elements::Break::new(1.0));
    doc.push(
        elements::Paragraph::new("Rust Reference Manual")
            .styled(style::Style::new().italic().with_font_size(14).with_color(style::Color::Rgb(100, 110, 120))),
    );
    doc.push(elements::Break::new(4.0));
    
    let local_time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    doc.push(
        elements::Paragraph::new(format!("Gerado em: {}", local_time))
            .styled(style::Style::new().with_font_size(10)),
    );
    
    doc.push(genpdf::elements::PageBreak::new());

    // Table of Contents
    doc.push(
        elements::Paragraph::new("Table of Contents")
            .styled(style::Style::new().bold().with_font_size(20)),
    );
    doc.push(elements::Break::new(1.0));
    
    let mut chapter_num = 1;
    
    doc.push(elements::Paragraph::new(format!("{}. Visão Geral da Crate", chapter_num)).styled(style::Style::new().bold().with_font_size(12)));
    chapter_num += 1;
    
    if !data.modules.is_empty() {
        doc.push(elements::Paragraph::new(format!("{}. Submódulos", chapter_num)).styled(style::Style::new().bold().with_font_size(12)));
        for m in &data.modules {
            doc.push(elements::Paragraph::new(format!("   • {}", m.name)).styled(style::Style::new().with_font_size(10)));
        }
        chapter_num += 1;
    }
    
    if !data.traits.is_empty() {
        doc.push(elements::Paragraph::new(format!("{}. Traits", chapter_num)).styled(style::Style::new().bold().with_font_size(12)));
        chapter_num += 1;
    }
    
    if !data.structs.is_empty() {
        doc.push(elements::Paragraph::new(format!("{}. Structs", chapter_num)).styled(style::Style::new().bold().with_font_size(12)));
        chapter_num += 1;
    }
    
    if !data.enums.is_empty() {
        doc.push(elements::Paragraph::new(format!("{}. Enums", chapter_num)).styled(style::Style::new().bold().with_font_size(12)));
        chapter_num += 1;
    }
    
    if !data.functions.is_empty() {
        doc.push(elements::Paragraph::new(format!("{}. Funções", chapter_num)).styled(style::Style::new().bold().with_font_size(12)));
        chapter_num += 1;
    }
    
    if !data.macros.is_empty() {
        doc.push(elements::Paragraph::new(format!("{}. Macros", chapter_num)).styled(style::Style::new().bold().with_font_size(12)));
        chapter_num += 1;
    }
    
    if !data.type_definitions.is_empty() {
        doc.push(elements::Paragraph::new(format!("{}. Type Definitions", chapter_num)).styled(style::Style::new().bold().with_font_size(12)));
        chapter_num += 1;
    }

    if !data.constants.is_empty() {
        doc.push(elements::Paragraph::new(format!("{}. Constants", chapter_num)).styled(style::Style::new().bold().with_font_size(12)));
        chapter_num += 1;
    }

    if !data.unions.is_empty() {
        doc.push(elements::Paragraph::new(format!("{}. Unions", chapter_num)).styled(style::Style::new().bold().with_font_size(12)));
    }
    
    doc.push(genpdf::elements::PageBreak::new());

    let h1_style = style::Style::new().bold().with_font_size(12);
    let mono_style = style::Style::new().with_font_family(mono_font_ref).with_font_size(8);
    
    let mut current_chap = 1;

    // Chapter 1: Overview
    doc.push(
        elements::Paragraph::new(format!("Capítulo {}: Visão Geral", current_chap))
            .styled(style::Style::new().bold().with_font_size(20)),
    );
    doc.push(elements::Break::new(1.0));
    render_doc_elements(&mut doc, &data.overview, mono_font_ref);
    current_chap += 1;
    
    // Chapter: Modules
    if !data.modules.is_empty() {
        doc.push(genpdf::elements::PageBreak::new());
        doc.push(
            elements::Paragraph::new(format!("Capítulo {}: Submódulos", current_chap))
                .styled(style::Style::new().bold().with_font_size(20)),
        );
        doc.push(elements::Break::new(1.0));
        
        for m in &data.modules {
            doc.push(
                elements::Paragraph::new(format!("module {}", m.name))
                    .styled(h1_style.with_color(style::Color::Rgb(79, 70, 229))),
            );
            doc.push(elements::Break::new(0.5));
            render_doc_elements(&mut doc, &m.overview, mono_font_ref);
            doc.push(elements::Break::new(1.5));
        }
        current_chap += 1;
    }

    let render_items = |doc: &mut Document, items: &[ItemDoc], title: &str, chap_num: usize, item_type_prefix: &str| {
        doc.push(genpdf::elements::PageBreak::new());
        doc.push(
            elements::Paragraph::new(format!("Capítulo {}: {}", chap_num, title))
                .styled(style::Style::new().bold().with_font_size(20)),
        );
        doc.push(elements::Break::new(1.0));

        for item in items {
            let full_name = if item.module_path.is_empty() {
                item.name.clone()
            } else {
                format!("{}::{}", item.module_path, item.name)
            };

            doc.push(
                elements::Paragraph::new(format!("{} {}", item_type_prefix, full_name))
                    .styled(h1_style.with_color(style::Color::Rgb(79, 70, 229))),
            );
            doc.push(elements::Break::new(0.4));

            if !item.decl.is_empty() {
                let mut decl_layout = elements::LinearLayout::vertical();
                for line in item.decl.lines() {
                    decl_layout.push(elements::Paragraph::new(line).styled(mono_style));
                }
                let padded = elements::PaddedElement::new(decl_layout, genpdf::Margins::trbl(3, 5, 3, 5));
                doc.push(elements::FramedElement::new(padded));
                doc.push(elements::Break::new(0.5));
            }

            render_doc_elements(doc, &item.docs, mono_font_ref);
            doc.push(elements::Break::new(1.5));
        }
    };

    if !data.traits.is_empty() {
        render_items(&mut doc, &data.traits, "Traits", current_chap, "trait");
        current_chap += 1;
    }

    if !data.structs.is_empty() {
        render_items(&mut doc, &data.structs, "Structs", current_chap, "struct");
        current_chap += 1;
    }

    if !data.enums.is_empty() {
        render_items(&mut doc, &data.enums, "Enums", current_chap, "enum");
        current_chap += 1;
    }

    if !data.functions.is_empty() {
        render_items(&mut doc, &data.functions, "Funções", current_chap, "fn");
        current_chap += 1;
    }

    if !data.macros.is_empty() {
        render_items(&mut doc, &data.macros, "Macros", current_chap, "macro");
        current_chap += 1;
    }

    if !data.type_definitions.is_empty() {
        render_items(&mut doc, &data.type_definitions, "Type Definitions", current_chap, "type");
        current_chap += 1;
    }

    if !data.constants.is_empty() {
        render_items(&mut doc, &data.constants, "Constants", current_chap, "const");
        current_chap += 1;
    }

    if !data.unions.is_empty() {
        render_items(&mut doc, &data.unions, "Unions", current_chap, "union");
    }

    doc.render_to_file(filename)?;
    Ok(())
}
