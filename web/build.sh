#!/bin/sh
rm -R ./node_modules/.vite
rm -R ./node_modules/logic-mesh
npm add ./module
npm run dev