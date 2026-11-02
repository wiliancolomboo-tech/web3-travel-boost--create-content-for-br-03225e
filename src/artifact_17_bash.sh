cp .env.example .env
# Edit RESEND_API_KEY and DISCORD_WEBHOOK_URL in .env
docker compose up -d --build
docker compose logs -f travelboost-oracle-worker