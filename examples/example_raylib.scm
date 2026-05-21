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

(define set-target-fps-sym (ffi-dlsym raylib "SetTargetFPS"))
(define set-target-fps
    (ffi-func set-target-fps-sym 'void '(i32)))

(define draw-circle-sym (ffi-dlsym raylib "DrawCircle"))
(define draw-circle
    (ffi-func draw-circle-sym 'void '(i32 i32 f32 i32)))

(define get-frame-time-sym (ffi-dlsym raylib "GetFrameTime"))
(define get-frame-time
    (ffi-func get-frame-time-sym 'f32 '()))

(define draw-fps-sym (ffi-dlsym raylib "DrawFPS"))
(define draw-fps
    (ffi-func draw-fps-sym 'void '(i32 i32)))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;

(define w 800)
(define h 600)

(init-window w h "Hello From Sel!")

(set-target-fps 60)

(define radius 50)
(define velocity 10)

(define ball {x 100 dx velocity y 100 dy velocity})

(define (collides-with-walls x y w h)
    (not (and (< 0 (- x radius))
              (< (+ x radius) w)
              (< 0 (- y radius))
              (< (+ y radius) h))))


(while (not (window-should-close))
    (define x  (rget ball 'x))
    (define dx (rget ball 'dx))
    (define y  (rget ball 'y))
    (define dy (rget ball 'dy))

    (define nx (+ x dx))
    (define ny (+ y dy))


    (if (collides-with-walls nx y w h)
        (set! ball (rset ball 'dx (* -1 dx)))
        (set! ball (rset ball 'x nx)))
    (if (collides-with-walls x ny w h)
        (set! ball (rset ball 'dy (* -1 dy)))
        (set! ball (rset ball 'y ny)))

    (begin-drawing)
        (clear-background 0xFF181818)
        (draw-circle (rget ball 'x) (rget ball 'y) radius 0xFF0000FF)
        (draw-fps 10 10)
    (end-drawing)
)


(close-window)
