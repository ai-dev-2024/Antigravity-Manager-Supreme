/**
 * 健壮的剪贴板复制工具函数
 * 
 * 浏览器限制：在非安全上下文（非 HTTPS 或 localhost）下，navigator.clipboard 是 undefined。
 * 本函数通过 execCommand('copy') 提供回退方案，确保在 HTTP 环境（如 Docker IP 访问）下也能正常工作。
 */
export async function copyToClipboard(text: string): Promise<boolean> {
    // 1. 尝试现代 Clipboard API
    if (navigator.clipboard && window.isSecureContext) {
        try {
            await navigator.clipboard.writeText(text);
            return true;
        } catch (err) {
            console.error('Clipboard API 复制失败:', err);
        }
    }

    // 2. 回退到传统的 execCommand('copy') 方案
    try {
        const textArea = document.createElement('textarea');
        textArea.value = text;

        // 确保 textarea 在页面上不可见，但必须在 DOM 中才能执行 copy
        textArea.style.position = 'fixed';
        textArea.style.left = '-9999px';
        textArea.style.top = '0';
        document.body.appendChild(textArea);

        textArea.focus();
        textArea.select();

        const successful = document.execCommand('copy');
        document.body.removeChild(textArea);

        return successful;
    } catch (err) {
        console.error('execCommand 复制失败:', err);
        return false;
    }
}
