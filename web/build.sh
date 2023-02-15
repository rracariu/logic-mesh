#!/bin/sh
rm -R ./node_modules/.vite
rm -R ./node_modules/logic-mesh
npm add ../pkg
npm run dev