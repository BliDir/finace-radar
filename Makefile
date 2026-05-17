.PHONY: up down build logs ps clean frontend backend check

up:
	docker compose up --build

down:
	docker compose down

build:
	docker compose build

logs:
	docker compose logs -f

ps:
	docker compose ps

clean:
	docker compose down -v

frontend:
	cd frontend && npm run dev

backend:
	cd backend && cargo run

check:
	cd frontend && npm run build
	cd backend && cargo check
