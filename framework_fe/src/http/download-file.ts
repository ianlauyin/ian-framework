import { networkError } from "./network";

// url is used as it is, with no prefix.
export async function downloadFile(url: string, name: string): Promise<void> {
  try {
    const response = await fetch(url, { credentials: "same-origin" });
    // Without this check, the body of a 403 or 404 response would be saved as the file.
    if (!response.ok) throw new Error(`HTTP ${response.status}`);

    const link = document.createElement("a");
    link.href = URL.createObjectURL(await response.blob());
    link.download = name;
    document.body.appendChild(link);
    link.click();
    URL.revokeObjectURL(link.href);
    link.remove();
  } catch (error) {
    throw networkError(error, url);
  }
}
