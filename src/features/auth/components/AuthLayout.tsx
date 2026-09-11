import type { ReactNode } from "react";
import AppTitleBar from "../../../app/components/AppTitleBar";
import BrandMark from "../../../components/ui/BrandMark";

type AuthLayoutProps = {
  eyebrow: string;
  title: string;
  description: string;
  children: ReactNode;
  background?: ReactNode;
};

export default function AuthLayout({
  eyebrow,
  title,
  description,
  children,
  background,
}: AuthLayoutProps) {
  return (
    <div className={`auth-shell${background ? " auth-shell-with-background" : ""}`}>
      <AppTitleBar />
      {background}
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
