(define empty (quote ()))
(define names (quote (alpha beta)))
(define nested (quote (quote inner)))
(define template (quasiquote (head (unquote value) (unquote-splicing rest))))
(define flags (list (quote #true) #false (quote on)))
(define long-form (quote a b))
(define vector-literal #(quote x))

(define-syntax pick
  (syntax-rules ()
    [(_ key) (hash-insert conf (quote key) #true)]))

(define commented (quote ;; a comment blocks the shorthand
                         value))
