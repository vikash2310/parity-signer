//
//  MetadataCard.swift
//  NativeSigner
//
//  Created by Alexander Slesarev on 12.8.2021.
//

import SwiftUI

struct MetadataCard: View {
    var meta: MMetadataRecord
    var body: some View {
        HStack {
            Image(uiImage: UIImage(data: Data(fromHexEncodedString: meta.metaIdPic) ?? Data()) ?? UIImage())
                .resizable(resizingMode: .stretch)
                .frame(width: 28, height: 28)
            VStack {
                Text("version")
                Text(meta.specsVersion)
            }
            VStack {
                Text("hash")
                Text(meta.metaHash.truncateMiddle(length: 8))
            }
        }.padding(.horizontal, 8)
    }
}

/*
struct MetadataCard_Previews: PreviewProvider {
    static var previews: some View {
        MetadataCard()
    }
}
*/
