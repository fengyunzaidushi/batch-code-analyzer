import { Database, ShieldCheck, Trash2, X } from "lucide-react";
import { useState } from "react";

interface DataManagementPanelProps {
  active: boolean;
  onClose: () => void;
  onResetAppData: () => Promise<void>;
}

export function DataManagementPanel({
  active,
  onClose,
  onResetAppData,
}: DataManagementPanelProps) {
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!active) return null;

  const reset = async () => {
    setBusy(true);
    setError(null);
    try {
      await onResetAppData();
      setConfirming(false);
      setError("已安排清理。请关闭并重新打开应用后生效。");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "本地数据清理失败。");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div aria-modal="true" className="data-management-backdrop" role="dialog">
      <section className="data-management-panel">
        <header>
          <div>
            <span className="data-management-icon" aria-hidden="true">
              <Database size={18} />
            </span>
            <div>
              <h2>本地数据管理</h2>
              <p>管理应用登记的数据，不修改仓库源代码。</p>
            </div>
          </div>
          <button
            aria-label="关闭数据管理"
            className="icon-button"
            onClick={onClose}
            title="关闭"
            type="button"
          >
            <X aria-hidden="true" size={18} />
          </button>
        </header>

        <div className="data-retention-note">
          <ShieldCheck aria-hidden="true" size={18} />
          <div>
            <strong>升级时保留</strong>
            <span>
              项目、提示词、扫描记录和运行历史保存在系统应用数据目录。
            </span>
          </div>
        </div>

        <div className="data-removal-row">
          <div>
            <strong>清空应用本地数据</strong>
            <span>
              删除项目登记、提示词、扫描记录、运行历史和本地 API 配置。
            </span>
            <small>
              仓库目录、`.batch-analysis` 配置镜像和结果文件会保留。
            </small>
          </div>
          {!confirming ? (
            <button
              className="danger-button"
              disabled={busy}
              onClick={() => setConfirming(true)}
              type="button"
            >
              <Trash2 aria-hidden="true" size={15} />
              清空本地数据
            </button>
          ) : (
            <div className="data-removal-actions">
              <button
                className="secondary-button"
                disabled={busy}
                onClick={() => setConfirming(false)}
                type="button"
              >
                取消
              </button>
              <button
                className="danger-button"
                disabled={busy}
                onClick={() => void reset()}
                type="button"
              >
                <Trash2 aria-hidden="true" size={15} />
                {busy ? "正在删除" : "确认删除"}
              </button>
            </div>
          )}
        </div>
        {error ? (
          <div className="project-error" role="alert">
            {error}
          </div>
        ) : null}
      </section>
    </div>
  );
}
