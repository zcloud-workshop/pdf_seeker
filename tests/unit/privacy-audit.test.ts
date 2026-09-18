import { describe, it, expect } from "vitest";
import { privacyAuditor } from "@/models/privacy";

describe("Privacy & Security Audit Model (Phase 2)", () => {
  it("initializes in verified 100% offline state with 0 bytes sent", () => {
    const status = privacyAuditor.getStatus();
    expect(status.isFullyOffline).toBe(true);
    expect(status.networkSentBytesTotal).toBe(0);
    expect(status.telemetryDisabled).toBe(true);
    expect(status.encryptedPdfProtectionActive).toBe(true);
    expect(status.transactionalAtomicWriteActive).toBe(true);
  });

  it("records local action in audit logs with 0 network bytes", () => {
    privacyAuditor.logAction("批量旋转", "contract.pdf");
    const status = privacyAuditor.getStatus();
    const latest = status.sessionRecords[0];

    expect(latest).toBeDefined();
    expect(latest.action).toBe("批量旋转");
    expect(latest.fileName).toBe("contract.pdf");
    expect(latest.networkSentBytes).toBe(0);
    expect(latest.destination).toBe("localhost (Tauri Sandbox)");
    expect(latest.status).toBe("verified_offline");
  });

  it("can clear audit logs cleanly", () => {
    privacyAuditor.clearAuditLogs();
    const status = privacyAuditor.getStatus();
    expect(status.sessionRecords.length).toBe(0);
  });

  it("supports real-time subscribe callback when logging actions", () => {
    let receivedCount = 0;
    const unsub = privacyAuditor.subscribe((st) => {
      receivedCount = st.sessionRecords.length;
    });

    privacyAuditor.logAction("打开本地文档", "test.pdf");
    expect(receivedCount).toBeGreaterThan(0);

    unsub();
  });

  it("runs privacy self-check and passes all 5 security criteria", () => {
    const checks = privacyAuditor.runPrivacySelfCheck();
    expect(checks.length).toBe(5);
    expect(checks.every((c) => c.passed)).toBe(true);
  });

  it("exports audit report in JSON and Markdown formats", () => {
    privacyAuditor.logAction("可控压缩优化", "sample.pdf");
    const md = privacyAuditor.exportLogs("markdown");
    const json = privacyAuditor.exportLogs("json");

    expect(md).toContain("本地隐私审计流水报告");
    expect(md).toContain("sample.pdf");

    const parsed = JSON.parse(json);
    expect(parsed.appName).toBe("PDF Seeker");
    expect(parsed.logs.length).toBeGreaterThan(0);
  });
});

