declare module 'extism:host' {
    interface user {
        get_memory(ptr: PTR): PTR;
    }
}

declare module 'main' {
    export function run(): I32;
}