#!/bin/sh
# Retained to fail safely for callers of the old installer endpoint.
printf '%s\n' 'Ion does not currently publish a Linux shell installer.' 'See the Ion website Downloads section for verified release availability.' >&2
exit 1
