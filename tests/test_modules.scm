(import mod_example)

(assert (eq? mod_example/a 10))
(assert (eq? mod_example/d 40))
(assert (eq? mod_example/f 50))

;; Test that private binding b is NOT exported (throws error)
(define test-private-b
  (lambda ()
    (try
      mod_example/b
      (catch err
        (begin
          (println "Caught expected private variable error: " err)
          'caught-private-b)))))

(assert (eq? (test-private-b) 'caught-private-b))

;; Test that private binding c is NOT exported (throws error)
(define test-private-c
  (lambda ()
    (try
      mod_example/c
      (catch err
        (begin
          (println "Caught expected private variable error: " err)
          'caught-private-c)))))

(assert (eq? (test-private-c) 'caught-private-c))

(println "Module visibility integration tests passed successfully!")
