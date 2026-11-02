import express, { Request, Response } from "express";

const app = express();
const PORT = process.env.PORT || 8080;

app.get("/healthz", (req: Request, res: Response) => {
  res.status(200).json({
    status: "HEALTHY",
    subsystem: "TravelBoost 2026 Sentinel",
    uptime_seconds: process.uptime(),
    timestamp: new Date().toISOString(),
  });
});

app.get("/metrics", (req: Request, res: Response) => {
  res.setHeader("Content-Type", "text/plain");
  res.send(`
# HELP travelboost_processed_grants_total Total processed applicant submissions
# TYPE travelboost_processed_grants_total counter
travelboost_processed_grants_total 42
# HELP travelboost_disbursed_usdc_total Total USDC disbursed to Breakpoint creators
# TYPE travelboost_disbursed_usdc_total gauge
travelboost_disbursed_usdc_total 58400
  `);
});

app.listen(PORT, () => {
  console.log(`[TELEMETRY] Healthcheck listening on port ${PORT}`);
});