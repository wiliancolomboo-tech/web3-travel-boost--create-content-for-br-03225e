cp .env.example .env
# Fill in ORACLE_PRIVATE_KEY, RESEND_API_KEY, SLACK_WEBHOOK_URL
docker compose up -d --build
docker compose ps
curl http://localhost:8080/health