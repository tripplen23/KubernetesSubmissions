FROM alpine:3.19
RUN apk add --no-cache curl
WORKDIR /app
COPY generate-todo.sh /generate-todo.sh
RUN chmod +x /generate-todo.sh
CMD ["/generate-todo.sh"]