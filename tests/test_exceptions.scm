;; Test exception try-catch system

;; 1. Successful try block
(define test-success
  (lambda ()
    (try
      42
      (catch err
        (begin
          (println "Failed success test!")
          99)))))

;; 2. Basic catch block on runtime error
(define test-catch
  (lambda ()
    (try
      (+ 1 "invalid-type") ;; Raises a runtime error: Attempt to perform arithmetic on non-number
      (catch err
        (begin
          (println "Caught expected error:")
          (println err)
          123)))))

;; 3. Deep unwinding test
(define deep-f3
  (lambda ()
    (+ 1 "error-deep")))

(define deep-f2
  (lambda ()
    (deep-f3)))

(define deep-f1
  (lambda ()
    (deep-f2)))

(define test-unwind
  (lambda ()
    (try
      (deep-f1)
      (catch err
        (begin
          (println "Unwound deep frames. Error:")
          (println err)
          777)))))

;; 4. Nested try-catch test
(define test-nested
  (lambda ()
    (try
      (try
        (+ 1 "inner-error")
        (catch inner-err
          (begin
            (println "Inner caught:")
            (println inner-err)
            555)))
      (catch outer-err
        (begin
          (println "Outer caught: should not be here!")
          999)))))

;; Run all tests
(define run-all-tests
  (lambda ()
    (begin
      (println "--- Exception Tests ---")
      (println (test-success))
      (println (test-catch))
      (println (test-unwind))
      (println (test-nested)))))

(run-all-tests)
