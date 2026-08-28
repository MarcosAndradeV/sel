;; Test the load builtin function

(println "Loading tests/helper_load.scm...")
(define res (load "helper_load.scm"))

(println "Checking return value of load...")
(assert (eq? res 42))

(println "Checking loaded variable...")
(assert (eq? loaded-var "hello-from-helper"))

(println "Checking loaded function...")
(assert (eq? (loaded-func 5) 50))

(println "Checking error cases (calling with non-existent file)...")
(try
  (begin
    (load "does_not_exist.scm")
    (assert #f)) ; Should not be reached
  (catch err
    (begin
      (println "Successfully caught error:" err)
      (assert #t))))

(println "Checking error cases (calling with non-string)...")
(try
  (begin
    (load 123)
    (assert #f)) ; Should not be reached
  (catch err
    (begin
      (println "Successfully caught error:" err)
      (assert #t))))

(println "Load builtin test passed successfully!")
