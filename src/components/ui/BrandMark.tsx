import keynestMark from "../../assets/keynest-mark.png";

type BrandMarkProps = {
  className?: string;
};

export default function BrandMark({ className = "" }: BrandMarkProps) {
  const classNames = ["brand-mark", className].filter(Boolean).join(" ");

  return (
    <img
      className={classNames}
      src={keynestMark}
      alt=""
      aria-hidden="true"
      draggable={false}
    />
  );
}
