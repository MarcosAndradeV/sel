;; Test System & OS Primitives

;; Environment variables
(set-env! "SEL_TEST_VAR" "sel_value_123")
(assert (eq? (get-env "SEL_TEST_VAR") "sel_value_123"))
(assert (nil? (get-env "SEL_NON_EXISTENT_VAR_XYZ")))

;; Time primitives
(define t1 (time-now-ms))
(assert (> t1 1000000))
(sleep-ms 10)
(define t2 (time-now-ms))
(assert (>= t2 t1))

;; Filesystem helpers
(define test-file "tests/temp_test_file.txt")
(when (fs-exists? test-file)
  (fs-delete test-file))

(assert (not (fs-exists? test-file)))
(fs-write test-file "Hello Sel Filesystem!")
(assert (fs-exists? test-file))
(assert (eq? (fs-read test-file) "Hello Sel Filesystem!"))

;; Listing directory
(define entries (fs-list "tests"))
(assert (list? entries))
(assert (> (count entries) 0))

;; Cleanup
(fs-delete test-file)
(assert (not (fs-exists? test-file)))
