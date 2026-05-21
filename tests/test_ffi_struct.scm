(define lib (ffi-dlopen "./libffi_test_structs.so"))

;; 1. Test passing struct by value
(define get-distance-sq-sym (ffi-dlsym lib "get_distance_sq"))
(define get-distance-sq (ffi-func get-distance-sq-sym 'f32 '((struct (f32 f32)))))

(println "Testing get_distance_sq:")
;; Point is passed as a list '(3.0 4.0)
(define d1 (get-distance-sq '(3.0 4.0)))
(println "Distance squared of (3.0 4.0) is: " d1)
(assert (= d1 25.0))

;; 2. Test returning struct by value
(define make-point-sym (ffi-dlsym lib "make_point"))
(define make-point (ffi-func make-point-sym '(struct (f32 f32)) '(f32 f32)))

(println "Testing make_point:")
(define p (make-point 5.0 12.0))
(println "Created point: " p)
(assert (eq? (car p) 5.0))
(assert (eq? (nth p 1) 12.0))

;; 3. Test nested structs passing
(define get-rect-area-sym (ffi-dlsym lib "get_rect_area"))
;; Rect: struct Point pos (struct f32 f32), float w, float h
(define get-rect-area (ffi-func get-rect-area-sym 'f32 '((struct ((struct (f32 f32)) f32 f32)))))

(println "Testing get_rect_area:")
;; Rect passed as nested list: '((10.0 20.0) 5.0 8.0)
(define area (get-rect-area '((10.0 20.0) 5.0 8.0)))
(println "Rect area is: " area)
(assert (= area 40.0))

;; 4. Test nested structs returning
(define make-rect-sym (ffi-dlsym lib "make_rect"))
(define make-rect (ffi-func make-rect-sym '(struct ((struct (f32 f32)) f32 f32)) '(f32 f32 f32 f32)))

(println "Testing make_rect:")
(define r (make-rect 1.0 2.0 10.0 20.0))
(println "Created rect: " r)
;; Expected: '((1.0 2.0) 10.0 20.0)
(define pos (car r))
(assert (eq? (car pos) 1.0))
(assert (eq? (nth pos 1) 2.0))
(assert (eq? (nth r 1) 10.0))
(assert (eq? (nth r 2) 20.0))

(println "All FFI struct tests passed successfully!")
