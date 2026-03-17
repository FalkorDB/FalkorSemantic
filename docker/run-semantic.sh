#!/bin/sh
# FalkorSemantic entrypoint — derived from FalkorDB v4.16.7 run.sh
# Adds --loadmodule for falkorsemantic.so AFTER falkordb.so (load order matters:
# FalkorSemantic's init checks that FalkorDB is already loaded via MODULE LIST)

if [ "${BROWSER:-1}" -eq "1" ]; then
    if [ -d "${FALKORDB_BROWSER_PATH}" ]; then
        cd "${FALKORDB_BROWSER_PATH}" && HOSTNAME="0.0.0.0" node server.js &
    fi
fi

mkdir -p "${FALKORDB_DATA_PATH}"

SEMANTIC_MODULE="${FALKORDB_BIN_PATH}/falkorsemantic.so"

if [ "${TLS:-0}" -eq "1" ]; then
    ${FALKORDB_BIN_PATH}/gen-certs.sh
    exec redis-server ${REDIS_ARGS} --protected-mode no \
        --tls-port 6379 --port 0 \
        --tls-cert-file ${FALKORDB_TLS_PATH}/redis.crt \
        --tls-key-file ${FALKORDB_TLS_PATH}/redis.key \
        --tls-ca-cert-file ${FALKORDB_TLS_PATH}/ca.crt \
        --tls-auth-clients no \
        --dir "${FALKORDB_DATA_PATH}" \
        --loadmodule "${FALKORDB_BIN_PATH}/falkordb.so" ${FALKORDB_ARGS} \
        --loadmodule "${SEMANTIC_MODULE}"
else
    exec redis-server ${REDIS_ARGS} --protected-mode no \
        --dir "${FALKORDB_DATA_PATH}" \
        --loadmodule "${FALKORDB_BIN_PATH}/falkordb.so" ${FALKORDB_ARGS} \
        --loadmodule "${SEMANTIC_MODULE}"
fi
