# Crate PDF Manual Generator (Tauri Luna Edition) 📦💾

[![Rust](https://img.shields.io/badge/rust-%23E34F26.svg?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/tauri-%2324C8DB.svg?style=flat&logo=tauri&logoColor=white)](https://tauri.app/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Uma aplicação desktop moderna e ultra-rápida construída em **Tauri v2** e **Rust** para buscar documentações de crates no crates.io e gerar manuais PDF perfeitamente estruturados e formatados offline, com suporte opcional a tradução automatizada para Português preservando as palavras-chave e sintaxe do Rust.

E o melhor: com a interface gráfica clássica do **Windows XP (Luna Blue)**, nostálgica, responsiva e leve!

---

## 🚀 Funcionalidades

- **Busca Direta:** Integração direta com a API pública do crates.io (sem necessidade de Selenium ou webdrivers pesados).
- **Gerador de PDF Estruturado:** Transforma a documentação HTML do docs.rs em um PDF hierárquico profissional com capa, sumário (Table of Contents), capítulos e blocos de código formatados.
- **Tradução com Preservação de Termos:** Traduz descrições e explicações para o Português através de um pipeline inteligente que protege termos e sintaxes do Rust (como `struct`, `impl`, `fn`, `Result`, etc.) impedindo que a tradução os desconfigure.
- **Interface Windows XP Luna:** UI nostálgica de alta fidelidade com barra de progresso verde segmentada, cronômetro de operação integrado e janela flutuante com sombra (frameless + transparente).
- **Sem Custos:** Feito para ser executado e consumido 100% gratuitamente.

---

## 🛠️ Como Executar

### Pré-requisitos
- **Rust toolchain** instalado ([rustup](https://rustup.rs/)).
- **Bibliotecas do WebKitGTK** (para Linux/Ubuntu):
  ```bash
  sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
  ```

### Rodando o Projeto em Desenvolvimento
```bash
cargo run
```

### Gerando o Build de Produção
```bash
cargo tauri build
```

---

## 🤝 Venha Contribuir! / Come Contribute! (Call for Contributors)

Este é um projeto de código aberto nascido da vontade de ter manuais físicos e digitais de consulta rápida e legível das nossas crates favoritas. E nós queremos a sua ajuda para torná-lo ainda melhor!

### 🇧🇷 Áreas onde você pode ajudar:
1. **Suporte Offline Real (`cargo doc` local):** Implementar um parser alternativo para documentações locais geradas via `cargo doc --no-deps`.
2. **Suporte a Outros Idiomas:** Estender o sistema de tradução inteligente para outras línguas além do Português.
3. **Novos Temas Clássicos (XP Classic, Royale, Zune):** Expandir as opções estéticas retro na UI.
4. **Melhorias de Layout no PDF:** Contribuir com melhorias nos estilos de tabelas e diagramas dentro do PDF gerado pelo `genpdf`.

---

### 🇱🇷 How you can help:
1. **Local Offline Parsing (`cargo doc`):** Build a pipeline to read locally generated rustdocs from standard compile targets.
2. **Multi-language Translation Support:** Generalize the protected-glossary translator to support other target languages.
3. **Retro Visual Themes:** Help us bring Windows Classic, Windows Vista Aero, or XP Zune/Royale themes to life!
4. **PDF Layout Rendering:** Optimize tables, borders, and custom fonts in the output documents.

Sinta-se livre para abrir **Issues** com bugs e sugestões, ou enviar **Pull Requests**!

---

## 📄 Licença

Distribuído sob a licença MIT. Veja `LICENSE` para mais informações.

---

*Criado com ❤️ por programadores apaixonados por Rust e pela era de ouro do desktop.*
