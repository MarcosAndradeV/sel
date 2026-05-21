(println "--- Testing OS Environment ---")
(define user (system 'getenv "USER"))
(println (if (nil? user) "USER not set" user))
(assert (nil? (system 'getenv "THIS_ENV_VAR_DOES_NOT_EXIST_12345")))

(println "--- Testing OS Args ---")
(println (system 'args))

(println "--- Testing File I/O ---")
(define filename "tests/test_output.txt")
(file-system 'write filename "Hello from sel Native I/O!")
(assert (file-system 'exists? filename))
(assert (eq? (file-system 'read filename) "Hello from sel Native I/O!"))

(println "--- Testing Missing File ---")
(assert (not (file-system 'exists? "tests/this_file_does_not_exist_404.txt")))

(println "--- Testing System Sleep ---")
(system 'sleep 1)
(println "Woke up!")
