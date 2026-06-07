import { LuTriangleAlert } from "react-icons/lu";

// A small confirmation modal — works everywhere (unlike window.confirm, which
// some webviews, e.g. Tauri's, ignore).
export const ConfirmModal: React.FC<{
  message: string;
  confirmText?: string;
  onConfirm: () => void;
  onCancel: () => void;
}> = ({ message, confirmText = "Delete", onConfirm, onCancel }) => (
  <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
    <div className="mx-4 w-full max-w-sm rounded-md bg-white text-gray-900 shadow-xl dark:bg-[#252526] dark:text-gray-100">
      <div className="flex items-start gap-3 p-5">
        <LuTriangleAlert className="mt-0.5 shrink-0 text-red-500" size={20} />
        <p className="text-sm">{message}</p>
      </div>
      <div className="flex justify-end gap-3 rounded-b-md bg-gray-50 p-4 dark:bg-[#2d2d2d]">
        <button
          type="button"
          onClick={onCancel}
          className="rounded-md border border-gray-300 px-4 py-2 text-sm text-gray-700 hover:bg-gray-100 dark:border-[#3a3a3a] dark:text-gray-200 dark:hover:bg-[#3a3a3a]"
        >
          Cancel
        </button>
        <button
          type="button"
          onClick={onConfirm}
          className="rounded-md bg-red-600 px-4 py-2 text-sm text-white hover:bg-red-700"
        >
          {confirmText}
        </button>
      </div>
    </div>
  </div>
);
