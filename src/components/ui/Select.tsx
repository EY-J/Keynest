import {
  type CSSProperties,
  type KeyboardEvent,
  type ReactNode,
  useEffect,
  useId,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";
import { Check, ChevronDown, X } from "lucide-react";
import "./select.css";

export interface SelectOption {
  value: string;
  label: string;
}

type SelectProps = {
  value: string;
  options: SelectOption[];
  onChange(value: string): void;
  onClear?: () => void;
  placeholder?: string;
  label?: string;
  ariaLabel?: string;
  disabled?: boolean;
  icon?: ReactNode;
  className?: string;
  menuClassName?: string;
  id?: string;
};

type MenuPosition = {
  top: number;
  left: number;
  width: number;
  maxHeight: number;
  transformOrigin: "top" | "bottom";
};

const CLOSE_DURATION_MS = 120;
const MENU_GAP = 6;
const MENU_PADDING = 6;
const OPTION_HEIGHT = 36;
const MAX_MENU_HEIGHT = 252;

export default function Select({
  value,
  options,
  onChange,
  onClear,
  placeholder = "Select",
  label,
  ariaLabel,
  disabled = false,
  icon,
  className = "",
  menuClassName = "",
  id,
}: SelectProps) {
  const generatedId = useId();
  const selectId = id ?? `keynest-select-${generatedId}`;
  const listboxId = `${selectId}-listbox`;
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const closeTimerRef = useRef<number | null>(null);
  const openRef = useRef(false);
  const [mounted, setMounted] = useState(false);
  const [open, setOpen] = useState(false);
  const selectedIndex = options.findIndex((option) => option.value === value);
  const [highlightedIndex, setHighlightedIndex] = useState(Math.max(selectedIndex, 0));
  const [position, setPosition] = useState<MenuPosition>({
    top: 0,
    left: 0,
    width: 0,
    maxHeight: MAX_MENU_HEIGHT,
    transformOrigin: "top",
  });

  openRef.current = open;
  const selectedOption = selectedIndex >= 0 ? options[selectedIndex] : null;
  const selectedValueSize = selectedOption
    ? selectedOption.label.length > 18
      ? "very-long"
      : selectedOption.label.length > 11
        ? "long"
        : undefined
    : undefined;

  function clearCloseTimer() {
    if (closeTimerRef.current !== null) {
      window.clearTimeout(closeTimerRef.current);
      closeTimerRef.current = null;
    }
  }

  function updatePosition() {
    const trigger = triggerRef.current;
    if (!trigger) return;

    const rect = trigger.getBoundingClientRect();
    const viewportHeight = window.visualViewport?.height ?? window.innerHeight;
    const viewportWidth = window.visualViewport?.width ?? window.innerWidth;
    const expectedHeight = Math.min(
      options.length * OPTION_HEIGHT + MENU_PADDING * 2,
      MAX_MENU_HEIGHT,
    );
    const roomBelow = viewportHeight - rect.bottom - MENU_GAP;
    const roomAbove = rect.top - MENU_GAP;
    const openAbove = roomBelow < expectedHeight && roomAbove > roomBelow;
    const menuHeight = Math.min(expectedHeight, openAbove ? roomAbove : roomBelow);
    const width = Math.min(rect.width, viewportWidth - 16);
    const left = Math.min(Math.max(8, rect.left), viewportWidth - width - 8);

    setPosition({
      top: openAbove
        ? Math.max(8, rect.top - Math.max(menuHeight, OPTION_HEIGHT + MENU_PADDING * 2) - MENU_GAP)
        : rect.bottom + MENU_GAP,
      left,
      width,
      maxHeight: Math.max(
        OPTION_HEIGHT + MENU_PADDING * 2,
        Math.min(MAX_MENU_HEIGHT, openAbove ? roomAbove : roomBelow),
      ),
      transformOrigin: openAbove ? "bottom" : "top",
    });
  }

  function openDropdown(initialIndex = selectedIndex >= 0 ? selectedIndex : 0) {
    if (disabled || options.length === 0) return;
    clearCloseTimer();
    updatePosition();
    setHighlightedIndex(Math.min(Math.max(initialIndex, 0), options.length - 1));
    setMounted(true);
    setOpen(true);
  }

  function closeDropdown() {
    if (!openRef.current) return;
    clearCloseTimer();
    setOpen(false);
    closeTimerRef.current = window.setTimeout(() => {
      setMounted(false);
      closeTimerRef.current = null;
    }, CLOSE_DURATION_MS);
  }

  function selectOption(index: number) {
    const option = options[index];
    if (!option) return;
    if (option.value !== value) onChange(option.value);
    closeDropdown();
    triggerRef.current?.focus();
  }

  function moveHighlight(amount: number) {
    setHighlightedIndex((current) => {
      const start = current >= 0 ? current : 0;
      return (start + amount + options.length) % options.length;
    });
  }

  function handleKeyDown(event: KeyboardEvent<HTMLButtonElement>) {
    if (disabled) return;

    if (event.key === "Tab") {
      closeDropdown();
      return;
    }
    if (event.key === "Escape" && open) {
      event.preventDefault();
      closeDropdown();
      return;
    }
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      if (!open) {
        openDropdown(
          selectedIndex >= 0
            ? selectedIndex
            : event.key === "ArrowDown" ? 0 : options.length - 1,
        );
      } else {
        moveHighlight(event.key === "ArrowDown" ? 1 : -1);
      }
      return;
    }
    if (event.key === "Home" || event.key === "End") {
      event.preventDefault();
      if (!open) openDropdown(event.key === "Home" ? 0 : options.length - 1);
      else setHighlightedIndex(event.key === "Home" ? 0 : options.length - 1);
      return;
    }
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      if (open) selectOption(highlightedIndex);
      else openDropdown();
    }
  }

  useEffect(() => {
    if (!mounted) return;
    const menu = menuRef.current;
    if (menu && "showPopover" in menu && !menu.matches(":popover-open")) {
      try { menu.showPopover(); } catch { /* Fixed-position fallback remains usable. */ }
    }
  }, [mounted]);

  useEffect(() => {
    if (!open) return;
    function handlePointerDown(event: PointerEvent) {
      const target = event.target as Node;
      if (!triggerRef.current?.contains(target) && !menuRef.current?.contains(target)) {
        closeDropdown();
      }
    }
    function reposition() { updatePosition(); }
    document.addEventListener("pointerdown", handlePointerDown, true);
    document.addEventListener("scroll", reposition, true);
    window.addEventListener("resize", reposition);
    window.visualViewport?.addEventListener("resize", reposition);
    return () => {
      document.removeEventListener("pointerdown", handlePointerDown, true);
      document.removeEventListener("scroll", reposition, true);
      window.removeEventListener("resize", reposition);
      window.visualViewport?.removeEventListener("resize", reposition);
    };
  }, [open, options.length]);

  useEffect(() => () => clearCloseTimer(), []);

  useEffect(() => {
    if (disabled) closeDropdown();
  }, [disabled]);

  useEffect(() => {
    if (!open) setHighlightedIndex(Math.max(selectedIndex, 0));
  }, [open, selectedIndex]);

  const menu = mounted ? createPortal(
    <div
      ref={menuRef}
      id={listboxId}
      className={`keynest-select__menu ${menuClassName}`.trim()}
      role="listbox"
      aria-label={ariaLabel ?? label}
      data-state={open ? "open" : "closing"}
      popover="manual"
      style={{
        top: position.top,
        left: position.left,
        width: position.width,
        maxHeight: position.maxHeight,
        "--keynest-select-origin": position.transformOrigin,
      } as CSSProperties}
    >
      {options.map((option, index) => (
        <div
          key={option.value}
          id={`${listboxId}-option-${index}`}
          className="keynest-select__option"
          role="option"
          aria-selected={option.value === value}
          data-highlighted={index === highlightedIndex ? "true" : undefined}
          onPointerMove={() => setHighlightedIndex(index)}
          onPointerDown={(event) => event.preventDefault()}
          onClick={() => selectOption(index)}
        >
          <span>{option.label}</span>
          {option.value === value ? <Check size={14} strokeWidth={2.4} aria-hidden="true" /> : null}
        </div>
      ))}
    </div>,
    document.body,
  ) : null;

  return (
    <div className={`keynest-select ${className}`.trim()}>
      {label ? <label className="keynest-select__label" htmlFor={selectId}>{label}</label> : null}
      <div
        className="keynest-select__control"
        data-clearable={onClear && selectedOption ? "true" : undefined}
        data-value-size={selectedValueSize}
      >
        <button
          ref={triggerRef}
          id={selectId}
          className="keynest-select__trigger"
          type="button"
          role="combobox"
          aria-label={ariaLabel}
          aria-expanded={open}
          aria-haspopup="listbox"
          aria-controls={mounted ? listboxId : undefined}
          aria-activedescendant={open ? `${listboxId}-option-${highlightedIndex}` : undefined}
          disabled={disabled}
          onClick={() => open ? closeDropdown() : openDropdown()}
          onKeyDown={handleKeyDown}
        >
          {icon ? <span className="keynest-select__icon" aria-hidden="true">{icon}</span> : null}
          <span
            className={selectedOption ? "keynest-select__value" : "keynest-select__placeholder"}
            title={selectedOption?.label}
          >
            {selectedOption?.label ?? placeholder}
          </span>
          <ChevronDown className="keynest-select__chevron" size={15} aria-hidden="true" />
        </button>
        {onClear && selectedOption ? (
          <button
            className="keynest-select__clear"
            type="button"
            aria-label={`Clear ${ariaLabel ?? label ?? "selection"}`}
            disabled={disabled}
            onClick={() => {
              closeDropdown();
              onClear();
              triggerRef.current?.focus();
            }}
          >
            <X size={13} strokeWidth={2.4} aria-hidden="true" />
          </button>
        ) : null}
      </div>
      {menu}
    </div>
  );
}
