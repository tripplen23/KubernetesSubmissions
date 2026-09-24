# curlimages/curl already carries curl and a shell; this adds the loop and
# nothing else. Both the init container and the sidecar speak to Wikipedia, so
# the image is the same family.
FROM curlimages/curl:8.10.0

USER root
COPY random-page.sh /usr/local/bin/random-page.sh
RUN chmod 0755 /usr/local/bin/random-page.sh
USER curl_user

# The image's own entrypoint is curl, which is not what this container runs.
ENTRYPOINT ["/usr/local/bin/random-page.sh"]
