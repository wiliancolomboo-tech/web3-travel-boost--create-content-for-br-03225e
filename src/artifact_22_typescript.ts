// Telemetry hook inside Sentinel daemon
import * as http from "http";

const server = http.createServer((req, res) => {
    if (req.url === "/healthz") {
        res.writeHead(200, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ status: "healthy", timestamp: Date.now() }));
    } else if (req.url === "/metrics") {
        res.writeHead(200, { "Content-Type": "text/plain" });
        res.end(`
# HELP travel_boost_submissions_total Total content submissions inspected
# TYPE travel_boost_submissions_total counter
travel_boost_submissions_total 42

# HELP travel_boost_disbursed_usdc_total Total USDC disbursed to Breakpoint grantees
# TYPE travel_boost_disbursed_usdc_total counter
travel_boost_disbursed_usdc_total 105000
        `.trim());
    } else {
        res.writeHead(404);
        res.end();
    }
});

server.listen(8080, () => {
    console.log("[Sentinel] Monitoring server active on port 8080");
});