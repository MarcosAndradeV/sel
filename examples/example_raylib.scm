; This program requires raylib 5.5 to be installed on the system. 
; You can install it using your package manager, or you can build it from source.
; Raylib Website: https://www.raylib.com/
; Raylib Github: https://github.com/raysan5/raylib

(define raylib (ffi-dlopen "libraylib.so"))

(define init-window-sym (ffi-dlsym raylib "InitWindow"))
(define init-window
    (ffi-func init-window-sym 'void '(i32 i32 *u8)))

(define close-window-sym (ffi-dlsym raylib "CloseWindow"))
(define close-window
    (ffi-func close-window-sym 'void '()))

(define begin-drawing-sym (ffi-dlsym raylib "BeginDrawing"))
(define begin-drawing
    (ffi-func begin-drawing-sym 'void '()))

(define end-drawing-sym (ffi-dlsym raylib "EndDrawing"))
(define end-drawing
    (ffi-func end-drawing-sym 'void '()))

(define window-should-close-sym (ffi-dlsym raylib "WindowShouldClose"))
(define window-should-close
    (ffi-func window-should-close-sym 'bool '()))

(define clear-background-sym (ffi-dlsym raylib "ClearBackground"))
(define clear-background
    (ffi-func clear-background-sym 'void '(i32)))

(init-window 800 600 "Hello From Sel!")

(define (loop)
    (when (not (window-should-close))
        (begin-drawing)

        (clear-background 0xFF0000FF)

        (end-drawing)
        (loop)
    )
)

(loop)

(close-window)
