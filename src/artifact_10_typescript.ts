import express from "express";
const app = express();
const PORT = process.env.PORT || 8080;

app.get("/health", (req, res) => {
  res.status(200).json({
    status: "healthy",
    oracle: "operational",
    cluster: "devnet",
    timestamp: new Date().toISOString(),
  });
});

app.listen(PORT, () => {
  console.log(`Healthcheck telemetry running on :${PORT}`);
});