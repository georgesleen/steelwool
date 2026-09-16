;single semicolon on its own line
;;;; four semicolons for a banner

(define (classify value) ;no space after the marker
;;    indented note
;;      - a list item that keeps its indentation
(cond [(zero? value) 'zero] [else 'other]))

(define layout #|  block comment left alone  |# 3)

;;@doc
;; A doc marker is program text, so it keeps its own marker.
(define counter 0) ;; two semicolons after code
;;
