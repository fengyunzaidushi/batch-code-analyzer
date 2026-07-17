import { Activity, CheckCircle2, RefreshCw, TriangleAlert } from "lucide-react";
import { useEffect, useState } from "react";
import type { HealthCheckResponse } from "@batch-code-analyzer/ipc-types";

import { checkBackendHealth } from "../ipc/health";

type HealthState = "checking" | HealthCheckResponse["status"];

export function App() {
  const [healthState, setHealthState] = useState<HealthState>("checking");
  const [requestId, setRequestId] = useState(0);

  useEffect(() => {
    let active = true;

    void checkBackendHealth().then(
      (response) => {
        if (active) setHealthState(response.status);
      },
      () => {
        if (active) setHealthState("unavailable");
      },
    );

    return () => {
      active = false;
    };
  }, [requestId]);

  return (
    <main className="app-shell">
      <header className="app-header">
        <div className="brand-mark" aria-hidden="true">
          <Activity size={20} strokeWidth={2} />
        </div>
        <div>
          <h1>Batch Code Analyzer</h1>
          <p>批量代码文件 AI 分析工具</p>
        </div>
      </header>

      <section className="health-panel" aria-labelledby="health-title">
        <div className="health-heading">
          <div>
            <p className="section-label">SYSTEM STATUS</p>
            <h2 id="health-title">桌面核心</h2>
          </div>
          <HealthIndicator state={healthState} />
        </div>

        {healthState === "degraded" || healthState === "unavailable" ? (
          <button
            className="retry-button"
            type="button"
            onClick={() => {
              setHealthState("checking");
              setRequestId((current) => current + 1);
            }}
          >
            <RefreshCw size={16} aria-hidden="true" />
            重新检查
          </button>
        ) : null}
      </section>
    </main>
  );
}

function HealthIndicator({ state }: { state: HealthState }) {
  if (state === "ready") {
    return (
      <div className="status status-ready" role="status">
        <CheckCircle2 size={18} aria-hidden="true" />
        本地核心已就绪
      </div>
    );
  }

  if (state === "unavailable") {
    return (
      <div className="status status-error" role="alert">
        <TriangleAlert size={18} aria-hidden="true" />
        无法连接本地核心
      </div>
    );
  }

  if (state === "degraded") {
    return (
      <div className="status status-degraded" role="status">
        <TriangleAlert size={18} aria-hidden="true" />
        本地核心降级
      </div>
    );
  }

  return (
    <div className="status status-checking" role="status">
      <RefreshCw className="spin" size={18} aria-hidden="true" />
      正在检查
    </div>
  );
}
