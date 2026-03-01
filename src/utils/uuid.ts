/**
 * Generates a UUID (Universally Unique Identifier) v4.
 * 
 * This function attempts to use the native `crypto.randomUUID()` API first.
 * If that API is unavailable (e.g., in non-secure contexts like HTTP),
 * it falls back to a custom implementation using checksum-based random generation.
 * 
 * @returns {string} A valid UUID v4 string.
 */
export const generateUUID = (): string => {
    // Try to use the native API first if available
    if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
        try {
            return crypto.randomUUID();
        } catch (e) {
            // Fallback if native call fails for some reason
            console.warn('crypto.randomUUID() failed, falling back to custom implementation', e);
        }
    }

    // Fallback implementation for non-secure contexts
    return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, function (c) {
        const r = Math.random() * 16 | 0;
        const v = c === 'x' ? r : (r & 0x3 | 0x8);
        return v.toString(16);
    });
};
