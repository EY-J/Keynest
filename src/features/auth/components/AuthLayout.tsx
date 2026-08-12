import type { ReactNode } from "react";
import AppTitleBar from "../../../shared/components/AppTitleBar";
import BrandMark from "../../../shared/components/BrandMark";

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
        <section className="auth-content" aria-labelledby="auth-title">
          <BrandMark className="auth-mark" />
          <p className="auth-eyebrow">{eyebrow}</p>
          <h1 id="auth-title">{title}</h1>
          <p className="auth-description">{description}</p>
          {children}
        </section>
      </main>
    </div>
  );
}
