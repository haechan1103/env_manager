import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { I18nProvider, localizeError, useI18n } from ".";

function LanguageProbe() {
  const { locale, setLocale, t } = useI18n();
  return (
    <div>
      <span>{locale}</span>
      <strong>{t("app.projectsTitle")}</strong>
      <button onClick={() => setLocale("ko")}>한국어</button>
    </div>
  );
}

describe("I18nProvider", () => {
  it("defaults to English and persists an explicit Korean selection", async () => {
    const user = userEvent.setup();
    render(
      <I18nProvider>
        <LanguageProbe />
      </I18nProvider>,
    );

    expect(screen.getByText("en")).toBeInTheDocument();
    expect(screen.getByText("Projects")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "한국어" }));

    expect(screen.getByText("ko")).toBeInTheDocument();
    expect(screen.getByText("프로젝트")).toBeInTheDocument();
    expect(window.localStorage.getItem("env-manager.locale")).toBe("ko");
    expect(document.documentElement.lang).toBe("ko");
  });

  it("uses stable backend error codes in English and preserves Korean details", () => {
    const backendError = {
      code: "FILE_CHANGED_EXTERNALLY",
      message: "파일이 외부에서 변경되었습니다: /private/project/.env",
    };

    expect(localizeError(backendError, "en", "error.unknown")).toBe(
      "The env file changed outside Env Manager. Refresh it and try again.",
    );
    expect(localizeError(backendError, "ko", "error.unknown")).toBe(
      backendError.message,
    );
  });
});
