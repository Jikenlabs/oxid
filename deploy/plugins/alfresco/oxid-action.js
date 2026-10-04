/**
 * Oxid Action Handler for Alfresco Share
 * Opens the document directly inside the high-performance Oxid viewer.
 */
(function() {
    YAHOO.Bubbling.fire("registerAction", {
        actionName: "onActionOxidView",
        fn: function onActionOxidView(record) {
            var nodeRef = record.nodeRef;
            var docId = nodeRef.replace("workspace://SpacesStore/", "");
            var filename = record.displayName || "document.pdf";

            // Alfresco Authentication Ticket
            var ticket = Alfresco.constants.ALF_TICKET || "";

            // URL of your deployed Oxid instance
            var oxidHost = window.OXID_URL || window.OXIDRENDER_URL || window.location.protocol + "//" + window.location.hostname + ":8080";

            // Open in modal window or full screen tab
            var viewerUrl = oxidHost + "/?connector=cmis&id=" + encodeURIComponent(docId) + "&token=" + encodeURIComponent(ticket) + "&filename=" + encodeURIComponent(filename);

            window.open(viewerUrl, "OxidViewer", "width=1200,height=800,resizable=yes,scrollbars=no,status=no");
        }
    });
})();
