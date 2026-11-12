import DOMPurify from 'dompurify';
import { marked } from 'marked';

export function renderReleaseNotes(markdown: string): string {
  return DOMPurify.sanitize(marked.parse(markdown, { async: false }), {
    USE_PROFILES: { html: true },
    // Release notes should not load remote media or interactive controls.
    FORBID_TAGS: ['img', 'video', 'audio', 'iframe', 'form', 'input', 'button', 'style'],
    FORBID_ATTR: ['style'],
  });
}
