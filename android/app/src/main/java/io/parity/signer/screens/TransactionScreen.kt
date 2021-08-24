package io.parity.signer.screens

import androidx.compose.material.Text
import androidx.compose.runtime.Composable

/**
 * This screen envelops scanning-signing workflow;
 * user only has up to 2 options at all times:
 *  - stop and go back
 *  - proceed
 *
 *  Sequence:
 *  1. Scanner
 *  2. Preview
 *  3. Signature
 */
@Composable
fun TransactionScreen() {
	Text(text = "Scanner goes brrr")
}
