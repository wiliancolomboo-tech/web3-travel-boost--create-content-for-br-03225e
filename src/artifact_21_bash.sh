cp .env.example .env
mkdir -p secrets
cp operator-wallet.json secrets/
docker compose up -d --build
curl http://localhost:8080/healthz
curl http://localhost:8080/readyz