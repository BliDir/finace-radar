# Finance Radar

Project layout:

- `frontend/` - Vite React dashboard. It requests Google OAuth access and displays backend data.
- `backend/` - Rust API backend for Gmail reads, AI classification, and database persistence.
- `docker-compose.yml` - Runs frontend, Rust backend, and Postgres together.

## Database

Use Postgres for this project.

Finance Radar needs relational, queryable data: Gmail messages, AI analysis audit records, normalized transactions, accounts, categories, monthly summaries, and future user/session tables. Postgres is a better fit than a document store because dashboard queries will frequently group/filter by month, account, category, merchant, and transaction direction.

The initial schema is in `backend/db/init.sql`.

## Environment

Use separate env files:

- `frontend/.env` - browser-safe Vite settings only, such as `VITE_GOOGLE_CLIENT_ID` and `VITE_API_BASE_URL`.
- `backend/.env` - backend-only settings and secrets, such as AI API keys, provider order, Ollama config, and `DATABASE_URL`.

Examples are available in `frontend/.env.example` and `backend/.env.example`.

## Run Everything

```bash
make up
```

Open:

- Frontend: `http://127.0.0.1:5175`
- Rust backend health: `http://127.0.0.1:4000/api/health`
- Postgres: `127.0.0.1:5432`

The frontend only displays data from the Rust backend. Gmail sync, AI analysis, and transaction persistence run in Rust.

If you already created the Postgres Docker volume before the amount columns changed, run `make clean` once before `make up` so the schema is recreated.

Useful commands:

```bash
make down
make logs
make ps
make clean
make check
```

## Frontend

```bash
cd frontend
npm run dev
```

## Rust Backend

```bash
cd backend
cargo run
```

The Rust backend exposes:

- `GET /api/health`
- `GET /api/config`
- `GET /api/dashboard?month=YYYY-MM`
- `POST /api/read-inbox`

`POST /api/read-inbox` accepts the selected month and a Google access token from the frontend, reads Gmail, analyzes each email with the configured AI provider chain, saves the results into Postgres, then returns the dashboard data.
