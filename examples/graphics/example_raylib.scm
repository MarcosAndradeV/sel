; This program requires raylib 5.5 to be installed on the system.
; You can install it using your package manager, or you can build it from source.
; Raylib Website: https://www.raylib.com/
; Raylib Github: https://github.com/raysan5/raylib

raylib := (ffi-dlopen "libraylib.so")

init-window-sym := (ffi-dlsym raylib "InitWindow")
init-window := (ffi-func init-window-sym 'void '(i32 i32 *u8))

close-window-sym := (ffi-dlsym raylib "CloseWindow")
close-window := (ffi-func close-window-sym 'void '())

begin-drawing-sym := (ffi-dlsym raylib "BeginDrawing")
begin-drawing := (ffi-func begin-drawing-sym 'void '())

end-drawing-sym := (ffi-dlsym raylib "EndDrawing")
end-drawing := (ffi-func end-drawing-sym 'void '())

window-should-close-sym := (ffi-dlsym raylib "WindowShouldClose")
window-should-close := (ffi-func window-should-close-sym 'bool '())

clear-background-sym := (ffi-dlsym raylib "ClearBackground")
clear-background := (ffi-func clear-background-sym 'void '(i32))

set-target-fps-sym := (ffi-dlsym raylib "SetTargetFPS")
set-target-fps := (ffi-func set-target-fps-sym 'void '(i32))

draw-circle-sym := (ffi-dlsym raylib "DrawCircle")
draw-circle := (ffi-func draw-circle-sym 'void '(i32 i32 f32 (struct (u8 u8 u8 u8))))

get-frame-time-sym := (ffi-dlsym raylib "GetFrameTime")
get-frame-time := (ffi-func get-frame-time-sym 'f32 '())

draw-fps-sym := (ffi-dlsym raylib "DrawFPS")
draw-fps := (ffi-func draw-fps-sym 'void '(i32 i32))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;

w := 800
h := 600

(init-window w h "Hello From Sel!")

(set-target-fps 60)

radius := 50
velocity := 10
color := { r 255 g 0 b 0 a 255 }

ball := {x 100 dx velocity y 100 dy velocity}

(define (collides-with-walls x y w h)
    (not (and (< 0 (- x radius))
              (< (+ x radius) w)
              (< 0 (- y radius))
              (< (+ y radius) h))))


(while (not (window-should-close))
    x  := (ball 'x)
    dx := (ball 'dx)
    y  := (ball 'y)
    dy := (ball 'dy)

    nx := (+ x dx)
    ny := (+ y dy)


    (set! ball (if (collides-with-walls nx y w h)
        (ball 'dx (* -1 dx))
        (ball 'x nx)))
    (set! ball (if (collides-with-walls x ny w h)
        (ball 'dy (* -1 dy))
        (ball 'y ny)))

    (begin-drawing)
        (clear-background 0xFF181818)
        (draw-circle (ball 'x) (ball 'y) radius color)
        (draw-fps 10 10)
    (end-drawing)
)

(close-window)
