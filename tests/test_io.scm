(println "--- Testing OS Environment ---")
(define user (system 'getenv "USER"))
(println (if (nil? user) "USER not set" user))

(println "--- Testing OS Args ---")
(println (system 'args))

(println "--- Testing File I/O ---")
(define filename "tests/test_output.txt")
(file-system 'write filename "Hello from sel Native I/O!")
(assert (file-system 'exists? filename))
(assert (eq? (file-system 'read filename) "Hello from sel Native I/O!"))
