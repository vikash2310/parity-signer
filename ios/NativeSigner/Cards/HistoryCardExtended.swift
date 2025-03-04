//
//  HistoryCardExtended.swift
//  NativeSigner
//
//  Created by Alexander Slesarev on 28.10.2021.
//

import SwiftUI

struct HistoryCardExtended: View {
    var event: EventDetailed
    let timestamp = ""
    var body: some View {
        HStack {
            switch event {
            case .databaseInitiated: HistoryCardTemplate(
                image: "iphone.and.arrow.forward",
                timestamp: timestamp,
                danger: false,
                line1: "Database initiated",
                line2: ""
            )
            case .deviceWasOnline: HistoryCardTemplate(
                image: "xmark.shield.fill",
                timestamp: timestamp,
                danger: true,
                line1: "Device was connected to network",
                line2: ""
            )
            case .generalVerifierSet(let value): HistoryCardTemplate(
                image: "checkmark.shield",
                timestamp: timestamp,
                danger: false,
                line1: "General verifier set",
                line2: value.public_key.truncateMiddle(length: 8) + "\n" + value.encryption
            )
            case .historyCleared: HistoryCardTemplate(
                image: "xmark.rectangle.portrait",
                timestamp: timestamp,
                danger: false,
                line1: "History cleared",
                line2: ""
            )
            case .identitiesWiped: HistoryCardTemplate(
                image: "xmark.rectangle.portrait",
                timestamp: timestamp,
                danger: false,
                line1: "All keys were wiped",
                line2: ""
            )
            case .identityAdded(let value): HistoryCardTemplate(
                image: "aqi.medium",
                timestamp: timestamp,
                danger: false,
                line1: "Key created",
                line2: value.seed_name.decode64() + value.path + " in network with hash " +  value.network_genesis_hash
            )
            case .identityRemoved(let value): HistoryCardTemplate(
                image: "xmark.rectangle.portrait",
                timestamp: timestamp,
                danger: false,
                line1: "Key removed",
                line2: value.seed_name.decode64() + value.path + " in network with hash " +  value.network_genesis_hash
            )
            case .metadataAdded(let value): HistoryCardTemplate(
                image: "plus.viewfinder",
                timestamp: timestamp,
                danger: false,
                line1: "Metadata added",
                line2: value.specname + " version " +  value.spec_version
            )
            case .metadataRemoved(let value): HistoryCardTemplate(
                image: "xmark.rectangle.portrait",
                timestamp: timestamp,
                danger: false,
                line1: "Metadata removed",
                line2: value.specname + " version " +  value.spec_version
            )
            case .networkAdded(let value): HistoryCardTemplate(
                image: "plus.viewfinder",
                timestamp: timestamp,
                danger: false,
                line1: "Network added",
                line2: value.title
            )
            case .networkRemoved(let value): HistoryCardTemplate(
                image: "xmark.rectangle.portrait",
                timestamp: timestamp,
                danger: false,
                line1: "Network removed",
                line2: value.title
            )
            case .networkVerifierSet(let value): HistoryCardTemplate(
                image: "checkmark.shield",
                timestamp: timestamp,
                danger: false,
                line1: "Network verifier set",
                line2: value.genesis_hash
            )
            case .resetDangerRecord: HistoryCardTemplate(
                image: "checkmark.shield",
                timestamp: timestamp,
                danger: true,
                line1: "Warnings acknowledged",
                line2: ""
            )
            case .seedCreated(let text):
                HistoryCardTemplate(
                    image: "aqi.medium",
                    timestamp: timestamp,
                    danger: false,
                    line1: "Seed created",
                    line2: text.decode64()
                )
            case .seedNameWasShown(let text): HistoryCardTemplate(
                image: "eye.trianglebadge.exclamationmark.fill",
                timestamp: timestamp,
                danger: false,
                line1: "Seed was shown",
                line2: text.decode64()
            )
            case .signedAddNetwork(let value): HistoryCardTemplate(
                image: "signature",
                timestamp: timestamp,
                danger: false,
                line1: "Network specs signed",
                line2: value.title
            )
            case .signedLoadMetadata(let value): HistoryCardTemplate(
                image: "signature",
                timestamp: timestamp,
                danger: false,
                line1: "Metadata signed",
                line2: value.specname + value.spec_version
            )
            case .signedTypes(_): HistoryCardTemplate(
                image: "signature",
                timestamp: timestamp,
                danger: false,
                line1: "Types signed",
                line2: ""
            )
            case .systemEntry(let text): HistoryCardTemplate(
                image: "eye.trianglebadge.exclamationmark.fill",
                timestamp: timestamp,
                danger: false,
                line1: "System record",
                line2: text
            )
            case .transactionSignError(let value): VStack {
                Text("Transaction failed")
                Text(value.error)
                TransactionBlock(cards: value.transaction.assemble())
                Text("Signed by: ")
                HStack {
                    Identicon(identicon: value.signed_by.identicon)
                    VStack {
                        Text(value.signed_by.hex)
                        Text(value.signed_by.encryption)
                    }
                }
                Text("in network")
                Text(value.network_name)
                Text("Comment :")
                Text(String(decoding: Data(base64Encoded: value.user_comment) ?? Data(), as: UTF8.self))
            }
            case .transactionSigned(let value):
                VStack {
                    Text("Transaction signed")
                    TransactionBlock(cards: value.transaction.assemble())
                    Text("Signed by: ")
                    HStack {
                        Identicon(identicon: value.signed_by.identicon)
                        VStack {
                            Text(value.signed_by.hex)
                            Text(value.signed_by.encryption)
                        }
                    }
                    Text("in network")
                    Text(value.network_name)
                    Text("Comment :")
                    Text(String(decoding: Data(base64Encoded: value.user_comment) ?? Data(), as: UTF8.self))
                }
            case .typesAdded(_): HistoryCardTemplate(
                image: "plus.viewfinder",
                timestamp: timestamp,
                danger: false,
                line1: "New types info loaded",
                line2: ""
            )
            case .typesRemoved(_): HistoryCardTemplate(
                image: "minus.square",
                timestamp: timestamp,
                danger: true,
                line1: "Types info removed",
                line2: ""
            )
            case .userEntry(let text): HistoryCardTemplate(
                image: "square",
                timestamp: timestamp,
                danger: false,
                line1: "User record",
                line2: text
            )
            case .warning(let text): HistoryCardTemplate(
                image: "exclamationmark.triangle.fill",
                timestamp: timestamp,
                danger: true,
                line1: "Warning! " + text,
                line2: ""
            )
            case .wrongPassword: HistoryCardTemplate(
                image: "exclamationmark.triangle.fill",
                timestamp: timestamp,
                danger: true,
                line1: "Wrong password entered",
                line2: "operation was declined"
            )
            case .messageSignError(let value): HistoryCardTemplate(
                image: "exclamationmark.triangle.fill",
                timestamp: timestamp,
                danger: true,
                line1: "Message signing error!",
                line2: value.error
            )
            case .messageSigned(let value): HistoryCardTemplate(
                image: "signature",
                timestamp: timestamp,
                danger: false,
                line1: "Generated signature for message",
                line2: String(decoding: Data(base64Encoded: value.user_comment) ?? Data(), as: UTF8.self)
            )
            }
        }
    }
}

/*
 struct HistoryCardExtended_Previews: PreviewProvider {
 static var previews: some View {
 HistoryCardExtended()
 }
 }
 */
