# W9 Search - AI RAG Web Application

A web application built with the **MASH** stack (Maud + Axum + SQLx + HTMX) that provides AI-powered search with Retrieval Augmented Generation (RAG) capabilities.

## Features

- 🤖 AI-powered answers using OpenRouter (DeepSeek R1T2 Chimera)
- 🔍 Web search integration with toggle on/off
- 📚 RAG system that retrieves and uses web sources
- 💾 SQLite database for storing sources
- 🎨 Minimalist but abnormal design aesthetic

## Tech Stack

- **Maud**: HTML templating
- **Axum**: Web framework
- **SQLx**: Database toolkit
- **HTMX**: Dynamic HTML interactions
- **OpenRouter**: AI model API

## Setup

1. Clone the repository and navigate to the project directory.

2. Create a `.env` file:
```bash
cp .env.example .env
```

3. Add your OpenRouter API key to `.env`:
```
OPENROUTER_API_KEY=your_key_here
```

4. Build and run:
```bash
cargo run
```

5. Open your browser to `http://localhost:3000`

## Usage

1. Enter your query in the text area
2. Toggle "WebSearch" on/off to enable/disable web search
3. Click "Query" to get AI-powered answers with source citations
4. Sources are automatically stored in the database for future queries

## Project Structure

```
src/
├── main.rs       # Application entry point
├── api.rs        # API handlers
├── db.rs         # Database operations
├── models.rs     # Data models
├── rag.rs        # RAG system implementation
├── search.rs     # Web search functionality
└── templates.rs  # Maud HTML templates
```

## License

MIT
