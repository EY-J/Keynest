import type { ReactNode } from "react";
import AppTitleBar from "../../../shared/components/AppTitleBar";

type AuthLayoutProps = {
  eyebrow: string;
  title: string;
  description: string;
  children: ReactNode;
};

export default function AuthLayout({
  eyebrow,
  title,
  description,
  children,
}: AuthLayoutProps) {
  return (
    <div className="auth-shell">
      <AppTitleBar />
      <main className="auth-page">
        <section className="auth-card" aria-labelledby="auth-title">
          <div className="auth-mark" aria-hidden="true">
            K
          </div>
          <p className="auth-eyebrow">{eyebrow}</p>
          <h1 id="auth-title">{title}</h1>
          <p className="auth-description">{description}</p>
          {children}
        </section>
      </main>
    </div>
  );
}
