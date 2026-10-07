// SPDX-License-Identifier: Apache-2.0
import { AlertDialog } from "@base-ui/react/alert-dialog";
import { Dialog as BaseDialog } from "@base-ui/react/dialog";
import { X } from "lucide-react";
import type { ReactNode } from "react";
import { Button } from "./button";

const backdrop =
  "fixed inset-0 bg-[color-mix(in_srgb,var(--foreground)_28%,transparent)] transition-opacity duration-(--duration-overlay) data-[ending-style]:opacity-0 data-[starting-style]:opacity-0";
const popup =
  "fixed top-[12vh] left-1/2 flex max-h-[76vh] w-[min(30rem,calc(100vw-2rem))] -translate-x-1/2 flex-col rounded-lg border border-border bg-raised text-foreground shadow-[0_18px_48px_-12px_rgba(28,25,23,0.35)] outline-none transition-[opacity,transform] duration-(--duration-overlay) data-[ending-style]:opacity-0 data-[starting-style]:translate-y-1 data-[starting-style]:opacity-0";

export interface DialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  /** One line under the title saying what the dialog is for. */
  description?: string;
  /** Label of the close button. */
  closeLabel: string;
  children: ReactNode;
  /** The dialog's buttons, primary last. */
  footer: ReactNode;
}

/** A modal dialog: focus stays inside, Escape closes, focus returns on close. */
export function Dialog({ open, onOpenChange, title, description, closeLabel, children, footer }: DialogProps) {
  return (
    <BaseDialog.Root open={open} onOpenChange={onOpenChange}>
      <BaseDialog.Portal>
        <BaseDialog.Backdrop className={backdrop} />
        <BaseDialog.Popup className={popup}>
          <div className="flex items-start gap-4 px-6 pt-5">
            <div className="min-w-0 flex-1">
              <BaseDialog.Title className="m-0 font-serif text-lg font-medium">{title}</BaseDialog.Title>
              {description ? (
                <BaseDialog.Description className="mt-1 text-sm text-muted-foreground">
                  {description}
                </BaseDialog.Description>
              ) : null}
            </div>
            <BaseDialog.Close
              aria-label={closeLabel}
              title={closeLabel}
              className="-mr-2 grid size-8 place-items-center rounded-md text-muted-foreground hover:bg-inset hover:text-foreground"
            >
              <X aria-hidden="true" className="size-4" />
            </BaseDialog.Close>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto px-6 py-5">{children}</div>
          <div className="flex justify-end gap-3 border-t border-border px-6 py-4">{footer}</div>
        </BaseDialog.Popup>
      </BaseDialog.Portal>
    </BaseDialog.Root>
  );
}

export interface ConfirmProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  /** What will happen, and whether it can be undone. */
  body: ReactNode;
  confirmLabel: string;
  cancelLabel: string;
  onConfirm: () => void;
  busy?: boolean;
  /** Destructive confirmations get the attention color. */
  destructive?: boolean;
}

/**
 * A confirmation, used only where `11-client-architecture.md` §4.2 requires
 * one. Focus starts on Cancel, so pressing Enter by habit changes nothing.
 */
export function Confirm({
  open,
  onOpenChange,
  title,
  body,
  confirmLabel,
  cancelLabel,
  onConfirm,
  busy,
  destructive,
}: ConfirmProps) {
  return (
    <AlertDialog.Root open={open} onOpenChange={onOpenChange}>
      <AlertDialog.Portal>
        <AlertDialog.Backdrop className={backdrop} />
        <AlertDialog.Popup className={popup}>
          <div className="px-6 pt-5 pb-5">
            <AlertDialog.Title className="m-0 font-serif text-lg font-medium">{title}</AlertDialog.Title>
            <AlertDialog.Description render={<div />} className="mt-2 text-sm">
              {body}
            </AlertDialog.Description>
          </div>
          <div className="flex justify-end gap-3 border-t border-border px-6 py-4">
            <AlertDialog.Close render={<Button variant="ghost" autoFocus />}>{cancelLabel}</AlertDialog.Close>
            <Button variant={destructive ? "destructive" : "primary"} busy={busy} onClick={onConfirm}>
              {confirmLabel}
            </Button>
          </div>
        </AlertDialog.Popup>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}
